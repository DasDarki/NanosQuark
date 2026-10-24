use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

use mlua::prelude::*;
use quark_core::{Options, Vec3, create_userdata, make_vector, options_and_callback, read_vector};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Integer(i64),
    Text(String),
}

impl Key {
    pub fn read(value: &LuaValue) -> LuaResult<Key> {
        match value {
            LuaValue::Integer(integer) => Ok(Key::Integer(*integer)),
            LuaValue::Number(number) if number.fract() == 0.0 => Ok(Key::Integer(*number as i64)),
            LuaValue::String(text) => Ok(Key::Text(text.to_str()?.to_owned())),
            other => Err(LuaError::runtime(format!(
                "node IDs are integers or strings, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Key::Integer(integer) => Ok(LuaValue::Integer(integer)),
            Key::Text(text) => lua.create_string(text).map(LuaValue::String),
        }
    }
}

#[derive(Clone, Default)]
struct GraphData {
    keys: Vec<Option<Key>>,
    positions: Vec<Option<Vec3>>,
    edges: Vec<Vec<(usize, f64)>>,
    lookup: HashMap<Key, usize>,
}

#[derive(Clone, Copy, PartialEq)]
struct Entry {
    priority: f64,
    node: usize,
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then(other.node.cmp(&self.node))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl GraphData {
    fn index(&self, key: &Key) -> Result<usize, String> {
        self.lookup.get(key).copied().ok_or_else(|| match key {
            Key::Integer(integer) => format!("the node {integer} does not exist"),
            Key::Text(text) => format!("the node '{text}' does not exist"),
        })
    }

    fn search(
        &self,
        start: usize,
        goal: Option<usize>,
        use_heuristic: bool,
        max_cost: f64,
    ) -> (Vec<f64>, Vec<usize>) {
        let count = self.keys.len();
        let mut distances = vec![f64::INFINITY; count];
        let mut previous = vec![usize::MAX; count];
        let mut open = BinaryHeap::new();
        let target = goal.and_then(|goal| self.positions[goal]);
        let heuristic = |node: usize| match (use_heuristic, target, self.positions[node]) {
            (true, Some(target), Some(position)) => target.subtract(position).length(),
            _ => 0.0,
        };
        distances[start] = 0.0;
        open.push(Entry {
            priority: heuristic(start),
            node: start,
        });
        while let Some(Entry { priority, node }) = open.pop() {
            if Some(node) == goal {
                break;
            }
            if priority - heuristic(node) > distances[node] + 1e-9 {
                continue;
            }
            for &(next, cost) in &self.edges[node] {
                let candidate = distances[node] + cost;
                if candidate < distances[next] && candidate <= max_cost {
                    distances[next] = candidate;
                    previous[next] = node;
                    open.push(Entry {
                        priority: candidate + heuristic(next),
                        node: next,
                    });
                }
            }
        }
        (distances, previous)
    }
}

pub struct Graph(Arc<GraphData>);

struct FoundPath {
    keys: Vec<Key>,
    cost: f64,
}

impl IntoLuaMulti for FoundPath {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<LuaMultiValue> {
        let list = lua.create_table_with_capacity(self.keys.len(), 0)?;
        for key in self.keys {
            list.raw_push(key.into_lua(lua)?)?;
        }
        (list, self.cost).into_lua_multi(lua)
    }
}

fn find(data: &GraphData, from: &Key, to: &Key, use_heuristic: bool) -> Result<FoundPath, String> {
    let start = data.index(from)?;
    let goal = data.index(to)?;
    let use_heuristic = use_heuristic && data.positions.iter().all(|position| position.is_some());
    let (distances, previous) = data.search(start, Some(goal), use_heuristic, f64::INFINITY);
    if !distances[goal].is_finite() {
        return Err("there is no path between the nodes".to_owned());
    }
    let mut path = vec![goal];
    let mut current = goal;
    while current != start {
        current = previous[current];
        path.push(current);
    }
    path.reverse();
    Ok(FoundPath {
        keys: path
            .into_iter()
            .filter_map(|node| data.keys[node].clone())
            .collect(),
        cost: distances[goal],
    })
}

impl LuaUserData for Graph {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut(
            "AddNode",
            |_, graph, (id, position): (LuaValue, Option<LuaValue>)| {
                let key = Key::read(&id)?;
                let position = position.map(|value| read_vector(&value)).transpose()?;
                let data = Arc::make_mut(&mut graph.0);
                match data.lookup.get(&key) {
                    Some(&index) => data.positions[index] = position,
                    None => {
                        data.lookup.insert(key.clone(), data.keys.len());
                        data.keys.push(Some(key));
                        data.positions.push(position);
                        data.edges.push(Vec::new());
                    }
                }
                Ok(())
            },
        );
        methods.add_method_mut("RemoveNode", |_, graph, id: LuaValue| {
            let key = Key::read(&id)?;
            let data = Arc::make_mut(&mut graph.0);
            let Some(index) = data.lookup.remove(&key) else {
                return Ok(false);
            };
            data.keys[index] = None;
            data.edges[index].clear();
            for edges in &mut data.edges {
                edges.retain(|(target, _)| *target != index);
            }
            Ok(true)
        });
        methods.add_method_mut(
            "AddEdge",
            |_, graph, (from, to, cost, options): (LuaValue, LuaValue, Option<f64>, Options)| {
                let data = Arc::make_mut(&mut graph.0);
                let start = data.index(&Key::read(&from)?).map_err(LuaError::runtime)?;
                let end = data.index(&Key::read(&to)?).map_err(LuaError::runtime)?;
                let cost = match (cost, data.positions[start], data.positions[end]) {
                    (Some(cost), _, _) => cost,
                    (None, Some(first), Some(second)) => first.subtract(second).length(),
                    (None, _, _) => 1.0,
                };
                if !(cost >= 0.0 && cost.is_finite()) {
                    return Err(LuaError::runtime(
                        "edge costs have to be finite numbers of at least 0",
                    ));
                }
                let mut connect = |a: usize, b: usize| {
                    data.edges[a].retain(|(target, _)| *target != b);
                    data.edges[a].push((b, cost));
                };
                connect(start, end);
                if options.get_or("Bidirectional", true)? {
                    connect(end, start);
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "RemoveEdge",
            |_, graph, (from, to, both): (LuaValue, LuaValue, Option<bool>)| {
                let data = Arc::make_mut(&mut graph.0);
                let start = data.index(&Key::read(&from)?).map_err(LuaError::runtime)?;
                let end = data.index(&Key::read(&to)?).map_err(LuaError::runtime)?;
                data.edges[start].retain(|(target, _)| *target != end);
                if both.unwrap_or(true) {
                    data.edges[end].retain(|(target, _)| *target != start);
                }
                Ok(())
            },
        );
        methods.add_method("HasNode", |_, graph, id: LuaValue| {
            Ok(graph.0.lookup.contains_key(&Key::read(&id)?))
        });
        methods.add_method("GetNodeCount", |_, graph, ()| Ok(graph.0.lookup.len()));
        methods.add_method("GetPosition", |lua, graph, id: LuaValue| {
            let index = graph.0.index(&Key::read(&id)?).map_err(LuaError::runtime)?;
            match graph.0.positions[index] {
                Some(position) => make_vector(lua, position),
                None => Ok(LuaNil),
            }
        });
        methods.add_method("GetNeighbors", |lua, graph, id: LuaValue| {
            let index = graph.0.index(&Key::read(&id)?).map_err(LuaError::runtime)?;
            let list = lua.create_table()?;
            for &(target, cost) in &graph.0.edges[index] {
                if let Some(key) = graph.0.keys[target].clone() {
                    let entry = lua.create_table()?;
                    entry.set("ID", key.into_lua(lua)?)?;
                    entry.set("Cost", cost)?;
                    list.raw_push(entry)?;
                }
            }
            Ok(list)
        });
        methods.add_method("FindNearest", |lua, graph, position: LuaValue| {
            let position = read_vector(&position)?;
            let best = graph
                .0
                .positions
                .iter()
                .enumerate()
                .filter(|(index, _)| graph.0.keys[*index].is_some())
                .filter_map(|(index, node)| {
                    node.map(|node| (index, node.subtract(position).length()))
                })
                .min_by(|left, right| left.1.total_cmp(&right.1));
            match best {
                Some((index, distance)) => (
                    graph.0.keys[index]
                        .clone()
                        .map(|key| key.into_lua(lua))
                        .transpose()?,
                    distance,
                )
                    .into_lua_multi(lua),
                None => LuaNil.into_lua_multi(lua),
            }
        });
        methods.add_method(
            "FindPath",
            |lua, graph, (from, to, options, callback): (LuaValue, LuaValue, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let from = Key::read(&from)?;
                let to = Key::read(&to)?;
                let use_heuristic: bool = options.get_or("Heuristic", true)?;
                let data = Arc::clone(&graph.0);
                quark_core::run(lua, callback, move || find(&data, &from, &to, use_heuristic))
            },
        );
        methods.add_method(
            "GetDistances",
            |lua, graph, (from, max_cost): (LuaValue, Option<f64>)| {
                let start = graph
                    .0
                    .index(&Key::read(&from)?)
                    .map_err(LuaError::runtime)?;
                let (distances, _) =
                    graph
                        .0
                        .search(start, None, false, max_cost.unwrap_or(f64::INFINITY));
                let table = lua.create_table()?;
                for (index, distance) in distances.into_iter().enumerate() {
                    if distance.is_finite()
                        && let Some(key) = graph.0.keys[index].clone()
                    {
                        table.raw_set(key.into_lua(lua)?, distance)?;
                    }
                }
                Ok(table)
            },
        );
    }
}

pub fn create(lua: &Lua, (): ()) -> LuaResult<LuaAnyUserData> {
    create_userdata(lua, Graph(Arc::new(GraphData::default())))
}
