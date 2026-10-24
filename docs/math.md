# Math

```lua
local Math, Geometry = Quark.Math, Quark.Geometry

local height = Math.Tween(0, 500, progress, "OutBounce")
local direction = Math.RotateVector(Vector(1, 0, 0), character:GetRotation())

local arena = Geometry.Cylinder(Vector(0, 0, 200), 3000, 400)
local spawn = arena:RandomPoint()
if not arena:Contains(player_position) then
    -- outside of the arena
end
```

Four modules: `Math` for interpolation, easing, curves, rotations, statistics and formulas, `Geometry` for shapes and polygons in 2D and 3D, `BigInt` for integers of any size and `Decimal` for exact decimal numbers.

## Vectors and rotators

Functions taking positions accept nanos world `Vector`, `Vector2D` and `Rotator` values, tables with `X`, `Y`, `Z` or `Pitch`, `Yaw`, `Roll` fields and lists like `{ x, y, z }`. Results are created with the nanos world `Vector`, `Vector2D` and `Rotator` when they exist, and are tables with the same fields otherwise. Rotations follow Unreal Engine: X is forward, Y right, Z up, angles are in degrees, `Yaw` turns around Z, `Pitch` around Y and `Roll` around X.

## Math

### Numbers

| Function | |
| --- | --- |
| `Math.Clamp(value, min, max)` | value limited to the range |
| `Math.Lerp(from, to, t)` | linear interpolation of numbers or vectors |
| `Math.InverseLerp(from, to, value)` | where `value` lies between `from` and `to`, 0 to 1 inside |
| `Math.Remap(value, from_min, from_max, to_min, to_max, clamp?)` | value moved from one range to another |
| `Math.SmoothStep(edge0, edge1, value)` | smooth 0 to 1 transition |
| `Math.MoveTowards(current, target, step)` | moves by at most `step` without overshooting |
| `Math.Round(value, decimals?)`, `Math.Sign(value)` | |
| `Math.Wrap(value, min, max)` | value wrapped into the range, like angles or looping indices |
| `Math.NormalizeAngle(angle)` | angle between -180 and 180 |
| `Math.DeltaAngle(from, to)`, `Math.LerpAngle(from, to, t)` | shortest difference and interpolation of angles |
| `Math.IsNearlyEqual(a, b, tolerance?)` | comparison with a tolerance (1e-8) |
| `Math.Pi`, `Math.Tau` | constants |

### Easing

`Math.Ease(name, t)` maps a progress from 0 to 1 to an eased progress, `Math.Tween(from, to, t, easing?)` interpolates numbers or vectors with it. `Math.GetEasings()` lists the names: `Linear` and `In`, `Out` and `InOut` versions of `Sine`, `Quad`, `Cubic`, `Quart`, `Quint`, `Expo`, `Circ`, `Back`, `Elastic` and `Bounce`, as shown on [easings.net](https://easings.net/).

```lua
local elapsed = Quark.Clock() - started
door:SetLocation(Math.Tween(closed, open, elapsed / 1.5, "InOutCubic"))
```

### Curves

| Function | |
| --- | --- |
| `Math.Bezier(points, t)` | point on a Bezier curve with any number of control points (numbers or vectors) |
| `Math.Spline(points, t, options?)` | point on a Catmull-Rom spline through all points, `t` from 0 to 1 over the whole path |
| `Math.SampleSpline(points, count, options?)` | `count` evenly spaced values of `t` along the spline |

Splines pass through every point, which makes them good for camera paths, patrol routes and race tracks. Options are `Closed` (connect the last point back to the first) and `Alpha`, 0.5 (centripetal, default) avoids loops and overshooting, 0 is uniform and 1 chordal.

### Rotations

| Function | |
| --- | --- |
| `Math.RotateVector(vector, rotation)` / `Math.UnrotateVector(vector, rotation)` | vector rotated by a rotator or quaternion, or back |
| `Math.GetDirections(rotation)` | forward, right and up vectors |
| `Math.LookAt(from, to)` | rotator looking from one position to another |
| `Math.SlerpRotator(from, to, t)` | smooth interpolation between rotators, without the jumps of interpolating angles |
| `Math.CombineRotators(first, second)` | rotation `first` followed by `second` |
| `Math.NormalizeRotator(rotator)` | all angles between -180 and 180 |
| `Math.Quaternion(x?, y?, z?, w?)` | quaternion, the identity without values |
| `Math.QuaternionFromRotator(rotator)`, `Math.QuaternionFromAxisAngle(axis, degrees)`, `Math.QuaternionBetween(from, to)` | quaternions from rotations or from the rotation turning one direction into another |
| `Math.IsQuaternion(value)` | |

Quaternions have the fields `X`, `Y`, `Z`, `W` and the methods `:RotateVector(vector)`, `:UnrotateVector(vector)`, `:Multiply(other)`, `:Inverse()`, `:Normalize()`, `:Slerp(other, t)`, `:Dot(other)`, `:AngleTo(other)` (degrees), `:ToRotator()`, `:ToAxisAngle()`, `:GetForward()`, `:GetRight()` and `:GetUp()`. `a * b` combines two quaternions (first `b`, then `a`), `quaternion * vector` rotates a vector.

### Statistics

`Math.Sum`, `Math.Mean`, `Math.Median`, `Math.Min`, `Math.Max`, `Math.Percentile(values, percent)`, `Math.Variance(values, sample?)` and `Math.StandardDeviation(values, sample?)` take a list of numbers or a [buffer view](buffer.md#views). With `sample = true` the variance divides by `n - 1`. Functions return `nil` for empty input. `Math.Summarize(values)` returns `{ Count, Sum, Mean, Min, Max, Median, P90, P99, StandardDeviation }` and `Math.Histogram(values, bins, min?, max?)` a list of counts per bin.

```lua
local summary = Math.Summarize(tick_times)
Console.Log(string.format("median %.2f ms, 99th percentile %.2f ms", summary.Median, summary.P99))
```

### Formulas

`Math.Evaluate(formula, variables?)` calculates a formula, `Math.Compile(formula)` checks and prepares it once for many evaluations. Both return `nil, message` for invalid formulas, which makes them safe for formulas from configuration files or admins: they can only calculate, never run code.

```lua
local damage = assert(Math.Compile("base * (1 + strength / 100) - armor"))
local value = damage:Evaluate({ base = 50, strength = 20, armor = 10 }) -- 50
```

| Syntax | |
| --- | --- |
| `+ - * / % ^` | arithmetic, `^` is right associative, `%` the floored remainder |
| `< <= > >= == !=` | comparisons, `1` for true and `0` for false |
| `and`, `or`, `not` | logic (also `&&`, `\|\|`, `!`) |
| `pi`, `e`, `inf`, `true`, `false` | constants |
| `abs sqrt cbrt exp ln log log10 log2 sin cos tan asin acos atan atan2 floor ceil round trunc sign min max clamp lerp pow hypot rad deg` | functions, `log(x, base?)`, `round(x, decimals?)`, `min` and `max` take any number of arguments |
| `if(condition, then, else)` | evaluates only the chosen branch |

Variables are numbers or booleans, a missing variable is reported when evaluating. Compiled formulas have `:Evaluate(variables?)`, can also be called directly (`damage(variables)`), and have `:GetVariables()` and `:GetSource()`.

## Geometry

### Shapes

Shapes answer whether a point is inside, give random points inside and find the closest point. 2D shapes work on `X` and `Y` and ignore `Z`, 3D shapes use all three.

| Function | |
| --- | --- |
| `Geometry.Circle(center, radius)` | 2D circle |
| `Geometry.Rectangle(center, extent, angle?)` | 2D rectangle, `extent` is half the size, `angle` in degrees |
| `Geometry.Polygon(points)` | 2D polygon, may be concave, without holes |
| `Geometry.Sphere(center, radius)` | |
| `Geometry.Box(center, extent, rotation?)` | box with half sizes `extent`, rotated by a rotator or quaternion |
| `Geometry.Cylinder(center, radius, height)` | upright cylinder around `center` |
| `Geometry.Capsule(start, finish, radius)` | all points within `radius` of a line, for example a character or a corridor |
| `Geometry.Prism(points, min_z, max_z)` | a 2D polygon between two heights, ideal for zones on a map |
| `Geometry.Union(shapes)` | several shapes of the same dimension acting as one |
| `Geometry.IsShape(value)` | |

| Method | |
| --- | --- |
| `:Contains(point)` | whether the point is inside or on the border |
| `:RandomPoint(generator?)` | evenly distributed random point inside, with a [generator](random.md#generators) for reproducible results |
| `:RandomPoints(count, generator?)` | list of random points |
| `:ClosestPoint(point)` | closest point inside the shape, the point itself when it is inside |
| `:Distance(point)` | distance to the shape, 0 inside |
| `:GetBounds()` | minimum and maximum corner of the bounding box |
| `:GetCenter()` | center, the centroid for polygons |
| `:GetArea()`, `:GetVolume()` | area of 2D shapes, volume of 3D shapes, `nil` for the other |
| `:GetType()`, `:Is3D()` | |

Points of 2D shapes are returned as `Vector2D`. Random points of a union pick a shape by its size, overlapping parts are therefore slightly more likely.

```lua
local zone = Geometry.Prism({ Vector2D(0, 0), Vector2D(5000, 0), Vector2D(5000, 3000), Vector2D(0, 3000) }, 0, 1000)
local loot_positions = zone:RandomPoints(20, Quark.Random.New("round-" .. round))
```

### Functions

| Function | |
| --- | --- |
| `Geometry.PointInPolygon(point, polygon)` | |
| `Geometry.PolygonArea(polygon, signed?)` | area, signed: positive for counter clockwise points |
| `Geometry.PolygonCentroid(polygon)` | |
| `Geometry.ConvexHull(points)` | smallest convex polygon around the points, counter clockwise |
| `Geometry.SegmentIntersection(a1, a2, b1, b2)` | intersection of two 2D segments, or `nil` |
| `Geometry.ClosestPointOnSegment(point, start, finish)`, `Geometry.DistanceToSegment(point, start, finish)` | 2D or 3D |
| `Geometry.Triangulate(polygon)` | triangles as lists of three point indices |
| `Geometry.Delaunay(points)` | Delaunay triangulation of a point set as index triples |
| `Geometry.SimplifyPath(points, tolerance)` | path with fewer points that deviates at most `tolerance` (Ramer-Douglas-Peucker) |
| `Geometry.BoundingBox(points, options?)` | minimum and maximum corner, option `Padding` |

## BigInt

Integers of any size, for example for large scores, IDs from other systems or cryptographic math. `BigInt.New(value)` takes an integer, a whole float, a text (`"123"`, `"-0x1f"`, `"0b1010"`) or a BigInt, `BigInt.Parse(text)` returns `nil, message` for invalid text instead of raising an error.

Arithmetic operators work with BigInts, numbers and texts on either side: `+ - * // % ^` and unary `-`. `/` is floored division like `//`, `%` takes the sign of the divisor like in Lua. `<`, `<=`, `>` and `>=` compare with numbers too, `==` only works between two BigInts, use `:Equals(value)` to compare with numbers. Results are limited to about one million bits.

| Method | |
| --- | --- |
| `:ToString(base?)` | text in base 2 to 36 (10), also `tostring` and `..` |
| `:ToNumber()`, `:FitsInteger()` | Lua integer when it fits, float otherwise |
| `:Abs()`, `:Pow(exponent)`, `:Sqrt()` | integer square root |
| `:ModPow(exponent, modulus)`, `:Gcd(other)`, `:Lcm(other)`, `:DivMod(other)` | |
| `:GetSign()`, `:IsZero()`, `:IsEven()`, `:GetBits()`, `:Compare(other)`, `:Equals(other)` | |

`BigInt.Factorial(n)` and `BigInt.IsBigInt(value)` complete the module.

## Decimal

Decimal numbers with up to 28 digits that calculate exactly, unlike floats where `0.1 + 0.2` is `0.30000000000000004`. Use them for money, prices and anything that is summed up a lot.

```lua
local price = Quark.Decimal.New("19.99")
local total = price * 3                       -- 59.97
local share = total:Split(4)                  -- four amounts that add up to exactly 59.97
Console.Log(total:Format({ Decimals = 2 }))   -- "59.97"
```

`Decimal.New(value)` takes a number, a text or a Decimal, `Decimal.Parse(text)` returns `nil, message` for invalid text. Floats are converted through their shortest text, so `Decimal.New(0.1)` is exactly `0.1`. Operators `+ - * / %`, unary `-` and comparisons work with numbers and texts too, `==` only between two Decimals. Results outside of the range and divisions by zero raise an error.

| Method | |
| --- | --- |
| `:Round(decimals?, mode?)` | rounding mode `HalfEven` (default, banker's rounding), `HalfUp`, `HalfDown`, `Up`, `Down`, `Ceil` or `Floor` |
| `:Truncate(decimals?)`, `:Floor()`, `:Ceil()`, `:Abs()`, `:Sqrt()`, `:Pow(exponent)` | |
| `:Format(options?)` | text with thousands separators, options `Decimals`, `Rounding`, `Separator` (`","`) and `Point` (`"."`) |
| `:Split(parts)` | splits into `parts` amounts at the current number of decimals that add up exactly, for example to divide a pot between players |
| `:ToString()`, `:ToNumber()` | also `tostring` and `..` |
| `:GetScale()`, `:IsZero()`, `:IsNegative()`, `:Compare(other)`, `:Equals(other)` | `GetScale` is the number of decimals |

`Decimal.Sum(values)` adds a list of numbers, texts and Decimals, `Decimal.IsDecimal(value)` checks the type.
