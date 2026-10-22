local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Random", function()
    local Random = Quark.Random

    Test.It("produces the same sequence for the same seed", function()
        local first, second = Random.New(42), Random.New(42)
        for _ = 1, 20 do
            Expect(first:Int(1, 1000000)):ToBe(second:Int(1, 1000000))
        end
        Expect(Random.New("world-1"):Float()):ToBe(Random.New("world-1"):Float())
        Expect(Random.New("world-1"):Float()).Not:ToBe(Random.New("world-2"):Float())
        Expect(Random.New(42):GetSeed()):ToBe(42)
        Expect(Random.HashSeed("world-1")):ToBe(Random.New("world-1"):GetSeed())
    end)

    Test.It("keeps known values stable across versions", function()
        local generator = Random.New(1)
        Expect(generator:GetState()):ToBe("910a2dec89025cc1beeb8da1658eec67f893a2eefb32555e71c18690ee42c90b")
        Expect(generator:Int(math.mininteger, math.maxinteger)):ToBe(-3475142291704528229)
        Expect(generator:Int(math.mininteger, math.maxinteger)):ToBe(-4665094578477473651)
        Expect(generator:Int(math.mininteger, math.maxinteger)):ToBe(1847458086238483744)
        Expect(Random.HashSeed("world-1")):ToBe(2415566837793212337)
    end)

    Test.It("generates integers and floats in range", function()
        local generator = Random.New(7)
        local seen = {}
        for _ = 1, 2000 do
            local value = generator:Int(-3, 3)
            Expect(math.type(value)):ToBe("integer")
            Expect(value >= -3 and value <= 3):ToBe(true)
            seen[value] = true
            local number = generator:Float(2, 5)
            Expect(number >= 2 and number < 5):ToBe(true)
        end
        for value = -3, 3 do
            Expect(seen[value]):ToBe(true)
        end
        local single = generator:Int(6)
        Expect(single >= 1 and single <= 6):ToBe(true)
        Expect(generator:Int(5, 5)):ToBe(5)
        Expect(function() generator:Int(5, 1) end):ToThrow("minimum")
        Expect(Random.Int(math.mininteger, math.maxinteger)):ToBeType("number")
    end)

    Test.It("uses a default generator per package", function()
        Random.Seed(99)
        local first = Random.Int(1, 1000000)
        Random.Seed(99)
        Expect(Random.Int(1, 1000000)):ToBe(first)
        Expect(Random.IsGenerator(Random.GetDefault())):ToBe(true)
        Expect(Random.IsGenerator({})):ToBe(false)
    end)

    Test.It("saves and restores the state", function()
        local generator = Random.New(5)
        generator:Int(1, 10)
        local state = generator:GetState()
        local expected = { generator:Int(1, 1000), generator:Float() }
        generator:SetState(state)
        Expect({ generator:Int(1, 1000), generator:Float() }):ToEqual(expected)

        local copy = generator:Clone()
        Expect(copy:Int(1, 1000000)):ToBe(generator:Int(1, 1000000))
        Expect(function() generator:SetState("nope") end):ToThrow("64 hexadecimal")
    end)

    Test.It("picks, shuffles and samples lists", function()
        local generator = Random.New(3)
        local item, index = generator:Pick({ "a", "b", "c" })
        Expect(({ a = 1, b = 2, c = 3 })[item]):ToBe(index)
        Expect(generator:Pick({})):ToBeNil()

        local list = { 1, 2, 3, 4, 5, 6, 7, 8 }
        local shuffled = generator:Shuffle(list)
        Expect(shuffled):ToBe(list)
        table.sort(list)
        Expect(list):ToEqual({ 1, 2, 3, 4, 5, 6, 7, 8 })

        local sample = generator:Sample({ 1, 2, 3, 4, 5 }, 3)
        Expect(sample):ToHaveLength(3)
        Expect(sample[1] ~= sample[2] and sample[2] ~= sample[3] and sample[1] ~= sample[3]):ToBe(true)
        Expect(generator:Sample({ 1, 2 }, 10)):ToHaveLength(2)
    end)

    Test.It("follows probabilities", function()
        local generator = Random.New(11)
        local hits = 0
        for _ = 1, 10000 do
            if generator:Bool(0.25) then
                hits = hits + 1
            end
        end
        Expect(hits / 10000):ToBeCloseTo(0.25, 0.02)
        Expect(generator:Bool(0)):ToBe(false)
        Expect(generator:Bool(1)):ToBe(true)
        Expect(function() generator:Bool(2) end):ToThrow("between 0 and 1")
    end)

    Test.It("samples distributions", function()
        local generator = Random.New(13)
        local count = 20000
        local normal, exponential, poisson, large_poisson, triangular = 0, 0, 0, 0, 0
        for _ = 1, count do
            normal = normal + generator:Normal(10, 2)
            exponential = exponential + generator:Exponential(4)
            poisson = poisson + generator:Poisson(3)
            large_poisson = large_poisson + generator:Poisson(50)
            local value = generator:Triangular(0, 10, 8)
            Expect(value >= 0 and value <= 10):ToBe(true)
            triangular = triangular + value
        end
        Expect(normal / count):ToBeCloseTo(10, 0.1)
        Expect(exponential / count):ToBeCloseTo(0.25, 0.02)
        Expect(poisson / count):ToBeCloseTo(3, 0.1)
        Expect(large_poisson / count):ToBeCloseTo(50, 0.5)
        Expect(triangular / count):ToBeCloseTo(6, 0.1)
        Expect(math.type(generator:Poisson(2))):ToBe("integer")
    end)

    Test.It("samples geometry", function()
        local generator = Random.New(17)
        for _ = 1, 500 do
            local x, y = generator:InsideCircle(5)
            Expect(x * x + y * y <= 25.0001):ToBe(true)
            local a, b, c = generator:OnSphere(2)
            Expect(math.sqrt(a * a + b * b + c * c)):ToBeCloseTo(2, 0.0001)
            local d, e, f = generator:InsideSphere()
            Expect(d * d + e * e + f * f <= 1.0001):ToBe(true)
        end
    end)

    Test.It("picks weighted items", function()
        local generator = Random.New(19)
        local counts = { common = 0, rare = 0, never = 0 }
        for _ = 1, 10000 do
            local item = generator:Weighted({ common = 9, rare = 1, never = 0 })
            counts[item] = counts[item] + 1
        end
        Expect(counts.rare / 10000):ToBeCloseTo(0.1, 0.02)
        Expect(counts.never):ToBe(0)
        Expect(function() generator:Weighted({ a = 0 }) end):ToThrow("greater than 0")
        Expect(function() generator:Weighted({ a = -1 }) end):ToThrow("at least 0")
    end)

    Test.It("rolls dice", function()
        local generator = Random.New(23)
        for _ = 1, 200 do
            local total, rolls = generator:Roll("2d6+3")
            Expect(rolls):ToHaveLength(2)
            Expect(total):ToBe(rolls[1] + rolls[2] + 3)
            Expect(total >= 5 and total <= 15):ToBe(true)
        end
        local total, rolls = generator:Roll("4d6kh3")
        local sorted = { table.unpack(rolls) }
        table.sort(sorted)
        Expect(total):ToBe(sorted[2] + sorted[3] + sorted[4])
        local percent = generator:Roll("d%")
        Expect(percent >= 1 and percent <= 100):ToBe(true)
        Expect(generator:Roll("10 - 1d1")):ToBe(9)
        Expect(function() generator:Roll("2x6") end):ToThrow("dice")
        Expect(function() generator:Roll("5000d6") end):ToThrow("1000")
    end)

    Test.It("rejects invalid seeds", function()
        Expect(function() Random.New({}) end):ToThrow("seed")
        Expect(function() Random.New(1.5) end):ToThrow("seed")
    end)
end)

Test.Describe("Random.WeightedPicker", function()
    local Random = Quark.Random

    Test.It("picks by weight", function()
        local picker = Random.WeightedPicker({ gold = 70, gem = 25, relic = 5 })
        Expect(picker:GetCount()):ToBe(3)
        Expect(picker:GetTotalWeight()):ToBe(100)
        Expect(picker:GetProbability("gem")):ToBeCloseTo(0.25, 0.0001)
        Expect(picker:GetProbability("missing")):ToBe(0)

        local generator = Random.New(29)
        local counts = { gold = 0, gem = 0, relic = 0 }
        for _, item in ipairs(picker:PickMany(20000, generator)) do
            counts[item] = counts[item] + 1
        end
        Expect(counts.gold / 20000):ToBeCloseTo(0.7, 0.02)
        Expect(counts.relic / 20000):ToBeCloseTo(0.05, 0.01)
    end)

    Test.It("accepts entry lists with any item", function()
        local sword, shield = { Name = "Sword" }, { Name = "Shield" }
        local picker = Random.WeightedPicker({ { Item = sword, Weight = 1 }, { shield, 3 } })
        local item = picker:Pick()
        Expect(item == sword or item == shield):ToBe(true)
        Expect(picker:GetProbability(shield)):ToBeCloseTo(0.75, 0.0001)
        Expect(picker:GetItems()[2]):ToEqual({ Item = shield, Weight = 3 })
    end)

    Test.It("is deterministic with a generator", function()
        local picker = Random.WeightedPicker({ a = 1, b = 2, c = 3, d = 4 })
        local first = picker:PickMany(50, Random.New(31))
        Expect(picker:PickMany(50, Random.New(31))):ToEqual(first)
        Expect(function() picker:Pick({}) end):ToThrow()
    end)
end)

Test.Describe("Random.LootTable", function()
    local Random = Quark.Random

    Test.It("rolls entries with counts and nothing entries", function()
        local loot = Random.LootTable({
            Rolls = { 2, 4 },
            Entries = {
                { Item = "coin", Weight = 6, Count = { 1, 5 } },
                { Item = "potion", Weight = 3 },
                { Weight = 1 },
            },
        })
        local generator = Random.New(37)
        for _ = 1, 100 do
            local drops = loot:Roll({ Generator = generator })
            Expect(#drops <= 4):ToBe(true)
            for _, drop in ipairs(drops) do
                Expect(drop.Item == "coin" or drop.Item == "potion"):ToBe(true)
                Expect(drop.Count >= 1 and drop.Count <= 5):ToBe(true)
            end
        end
        Expect(loot:GetChance("potion")):ToBeCloseTo(0.3, 0.0001)
    end)

    Test.It("merges drops and handles guaranteed and nested tables", function()
        local gems = Random.LootTable({ Entries = { { Item = "ruby" }, { Item = "emerald" } } })
        local loot = Random.LootTable({
            Rolls = 10,
            Guaranteed = { { Item = "xp", Count = 50 }, { Item = "never", Chance = 0 } },
            Entries = { { Item = "coin" }, { Table = gems } },
        })
        local drops = loot:Roll({ Generator = Random.New(41), Merge = true })
        local total = 0
        local seen = {}
        for _, drop in ipairs(drops) do
            Expect(seen[drop.Item]):ToBeNil()
            seen[drop.Item] = true
            if drop.Item ~= "xp" then
                total = total + drop.Count
            end
        end
        Expect(drops[1]):ToEqual({ Item = "xp", Count = 50 })
        Expect(seen.never):ToBeNil()
        Expect(total):ToBe(10)
    end)

    Test.It("guarantees items through pity", function()
        local loot = Random.LootTable({
            Entries = { { Item = "junk", Weight = 1000000 }, { Item = "legendary", Weight = 0.000001 } },
            Pity = { { Item = "legendary", After = 10 } },
        })
        local pity = {}
        local generator = Random.New(43)
        for roll = 1, 30 do
            local item = loot:Roll({ Generator = generator, Pity = pity })[1].Item
            Expect(item):ToBe(roll % 10 == 0 and "legendary" or "junk")
        end
        Expect(pity.legendary):ToBe(0)
    end)

    Test.It("validates definitions", function()
        Expect(function() Random.LootTable({ Rolls = -1 }) end):ToThrow("Rolls")
        Expect(function() Random.LootTable({ Entries = { { Item = "a", Weight = -1 } } }) end):ToThrow("at least 0")
        Expect(function() Random.LootTable({ Entries = { { Item = "a" } }, Pity = { { Item = "b", After = 3 } } }) end):ToThrow("not in Entries")
        Expect(function() Random.LootTable({ Entries = { { Table = {} } } }) end):ToThrow("not a loot table")
        Expect(#Random.LootTable({}):Roll()):ToBe(0)
    end)
end)

Test.Describe("Random.Noise", function()
    local Random = Quark.Random

    Test.It("samples deterministic noise", function()
        local noise = Random.Noise({ Seed = "terrain", Type = "Perlin", Fractal = "FBm", Octaves = 4, Frequency = 0.02 })
        local value = noise:Get(10.5, 20.25)
        Expect(value >= -1 and value <= 1):ToBe(true)
        Expect(Random.Noise({ Seed = "terrain", Type = "Perlin", Fractal = "FBm", Octaves = 4, Frequency = 0.02 }):Get(10.5, 20.25)):ToBe(value)
        Expect(noise:Get(1, 2, 3)):ToBeType("number")
        Expect(function() Random.Noise({ Type = "Plasma" }) end):ToThrow("unknown noise type")
    end)

    Test.It("fills areas into buffers", function()
        local noise = Random.Noise({ Seed = 5, Type = "Cellular" })
        local buffer = noise:Fill(8, 4, { X = 100, Y = 50, Step = 2 })
        Expect(buffer:GetSize()):ToBe(8 * 4 * 4)
        local base = buffer:GetIndexBase()
        Expect(buffer:Read("f32le", base + (1 * 8 + 3) * 4)):ToBeCloseTo(noise:Get(100 + 3 * 2, 50 + 1 * 2), 0.00001)

        local async = Quark.Await(noise.Fill, noise, 8, 4, { X = 100, Y = 50, Step = 2 })
        Expect(async:GetBytes()):ToBe(buffer:GetBytes())
        Expect(function() noise:Fill(0, 4) end):ToThrow("samples")
    end)
end)
