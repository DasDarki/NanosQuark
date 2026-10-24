local Test = Quark.Import("Testing")
local Expect = Test.Expect

local function near(vector, x, y, z)
    Expect(vector.X):ToBeCloseTo(x, 0.0001)
    Expect(vector.Y):ToBeCloseTo(y, 0.0001)
    if z then
        Expect(vector.Z):ToBeCloseTo(z, 0.0001)
    end
end

Test.Describe("Math", function()
    local Math = Quark.Math

    Test.It("clamps, interpolates and remaps", function()
        Expect(Math.Clamp(15, 0, 10)):ToBe(10)
        Expect(Math.Lerp(10, 20, 0.25)):ToBe(12.5)
        near(Math.Lerp({ X = 0, Y = 0, Z = 0 }, { X = 10, Y = 20, Z = 30 }, 0.5), 5, 10, 15)
        Expect(Math.InverseLerp(10, 20, 15)):ToBe(0.5)
        Expect(Math.Remap(5, 0, 10, 100, 200)):ToBe(150)
        Expect(Math.Remap(20, 0, 10, 100, 200, true)):ToBe(200)
        Expect(Math.SmoothStep(0, 1, 0.5)):ToBe(0.5)
        Expect(Math.MoveTowards(0, 10, 3)):ToBe(3)
        Expect(Math.Round(3.14159, 2)):ToBe(3.14)
        Expect(Math.Sign(-4)):ToBe(-1)
        Expect(Math.Wrap(370, 0, 360)):ToBe(10)
        Expect(Math.NormalizeAngle(270)):ToBe(-90)
        Expect(Math.DeltaAngle(350, 10)):ToBe(20)
        Expect(Math.LerpAngle(350, 10, 0.5)):ToBe(360)
        Expect(Math.IsNearlyEqual(0.1 + 0.2, 0.3)):ToBe(true)
    end)

    Test.It("eases values", function()
        Expect(Math.Ease("OutQuad", 0.5)):ToBe(0.75)
        Expect(Math.Ease("InOutCubic", 0.5)):ToBe(0.5)
        for _, name in ipairs(Math.GetEasings()) do
            Expect(Math.Ease(name, 0)):ToBeCloseTo(0, 0.000001)
            Expect(Math.Ease(name, 1)):ToBeCloseTo(1, 0.000001)
        end
        Expect(Math.Tween(0, 100, 0.5, "OutQuad")):ToBe(75)
        Expect(function() Math.Ease("Wobble", 0.5) end):ToThrow("unknown easing")
    end)

    Test.It("evaluates curves", function()
        near(Math.Bezier({ { X = 0, Y = 0 }, { X = 1, Y = 2 }, { X = 2, Y = 0 } }, 0.5), 1, 1)
        Expect(Math.Bezier({ 0, 10 }, 0.3)):ToBeCloseTo(3, 0.0001)
        local points = { { X = 0, Y = 0, Z = 0 }, { X = 100, Y = 0, Z = 0 }, { X = 100, Y = 100, Z = 0 }, { X = 0, Y = 100, Z = 50 } }
        near(Math.Spline(points, 0), 0, 0, 0)
        near(Math.Spline(points, 1 / 3), 100, 0, 0)
        near(Math.Spline(points, 1), 0, 100, 50)
        local samples = Math.SampleSpline(points, 10, { Closed = true })
        Expect(#samples):ToBe(10)
        near(samples[1], 0, 0, 0)
        Expect(function() Math.Spline({ points[1] }, 0.5) end):ToThrow("at least 2 points")
    end)

    Test.It("rotates with quaternions like Unreal", function()
        local yaw = Math.QuaternionFromRotator({ Pitch = 0, Yaw = 90, Roll = 0 })
        near(yaw:RotateVector({ X = 1, Y = 0, Z = 0 }), 0, 1, 0)
        near(Math.RotateVector({ X = 1, Y = 0, Z = 0 }, { Pitch = 90, Yaw = 0, Roll = 0 }), 0, 0, 1)
        local forward, right, up = Math.GetDirections({ Pitch = 0, Yaw = 90, Roll = 0 })
        near(forward, 0, 1, 0)
        near(right, -1, 0, 0)
        near(up, 0, 0, 1)

        local rotator = Math.QuaternionFromRotator({ Pitch = 30, Yaw = -45, Roll = 10 }):ToRotator()
        Expect(rotator.Pitch):ToBeCloseTo(30, 0.0001)
        Expect(rotator.Yaw):ToBeCloseTo(-45, 0.0001)
        Expect(rotator.Roll):ToBeCloseTo(10, 0.0001)

        local look = Math.LookAt({ X = 0, Y = 0, Z = 0 }, { X = 0, Y = 100, Z = 100 })
        Expect({ Math.Round(look.Pitch, 4), Math.Round(look.Yaw, 4) }):ToEqual({ 45, 90 })
        local halfway = Math.SlerpRotator({ Pitch = 0, Yaw = 0, Roll = 0 }, { Pitch = 0, Yaw = 90, Roll = 0 }, 0.5)
        Expect(halfway.Yaw):ToBeCloseTo(45, 0.0001)

        local axis = Math.QuaternionFromAxisAngle({ X = 0, Y = 0, Z = 1 }, 90)
        Expect(axis:AngleTo(yaw)):ToBeCloseTo(0, 0.0001)
        near((axis * axis):RotateVector({ X = 1, Y = 0, Z = 0 }), -1, 0, 0)
        near(axis * { X = 0, Y = 1, Z = 0 }, -1, 0, 0)
        near(axis:Inverse():RotateVector(axis:RotateVector({ X = 3, Y = 4, Z = 5 })), 3, 4, 5)
        local between = Math.QuaternionBetween({ X = 1, Y = 0, Z = 0 }, { X = 0, Y = 0, Z = 1 })
        near(between:RotateVector({ X = 2, Y = 0, Z = 0 }), 0, 0, 2)
        local identity = Math.Quaternion()
        Expect({ identity.X, identity.Y, identity.Z, identity.W }):ToEqual({ 0, 0, 0, 1 })
        Expect(Math.IsQuaternion(identity)):ToBe(true)
        Expect(Math.NormalizeRotator({ Pitch = 370, Yaw = -190, Roll = 0 }).Yaw):ToBe(170)
    end)

    Test.It("creates nanos world Vectors and Rotators when available", function()
        Vector = function(x, y, z) return { X = x, Y = y, Z = z, Kind = "Vector" } end
        Rotator = function(pitch, yaw, roll) return { Pitch = pitch, Yaw = yaw, Roll = roll, Kind = "Rotator" } end
        local ok, message = pcall(function()
            Expect(Math.RotateVector({ 1, 0, 0 }, { 0, 90, 0 }).Kind):ToBe("Vector")
            Expect(Math.LookAt({ 0, 0, 0 }, { 1, 0, 0 }).Kind):ToBe("Rotator")
        end)
        Vector, Rotator = nil, nil
        Expect(ok):ToBe(true, message)
    end)

    Test.It("computes statistics", function()
        local values = { 2, 4, 4, 4, 5, 5, 7, 9 }
        Expect(Math.Sum(values)):ToBe(40)
        Expect(Math.Mean(values)):ToBe(5)
        Expect(Math.Median({ 1, 3, 2, 4 })):ToBe(2.5)
        Expect(Math.StandardDeviation(values)):ToBe(2)
        Expect(Math.Variance(values, true)):ToBeCloseTo(4.5714, 0.0001)
        Expect(Math.Percentile({ 1, 2, 3, 4, 5 }, 90)):ToBeCloseTo(4.6, 0.0001)
        Expect({ Math.Min(values), Math.Max(values) }):ToEqual({ 2, 9 })
        Expect(Math.Histogram({ 1, 2, 2, 3, 9 }, 2)):ToEqual({ 4, 1 })
        Expect(Math.Mean({})):ToBeNil()
        local summary = Math.Summarize(values)
        Expect({ summary.Count, summary.Median, summary.Max }):ToEqual({ 8, 4.5, 9 })

        local view = Quark.Buffer.New(12):View("f32")
        view:FromTable({ 1, 2, 6 })
        Expect(Math.Mean(view)):ToBe(3)
        Expect(function() Math.Sum({ 1, "x" }) end):ToThrow("list of numbers")
    end)

    Test.It("evaluates expressions", function()
        Expect(Math.Evaluate("2 * x + 1", { x = 3 })):ToBe(7)
        Expect(Math.Evaluate("2 ^ 3 ^ 2")):ToBe(512)
        Expect(Math.Evaluate("-2 ^ 2")):ToBe(-4)
        Expect(Math.Evaluate("max(1, 5, 3) + min(4, 2)")):ToBe(7)
        Expect(Math.Evaluate("if(level >= 10 and vip, 2, 1) * 100", { level = 12, vip = true })):ToBe(200)
        Expect(Math.Evaluate("7 % 3 + floor(2.7) + round(1.256, 2)")):ToBeCloseTo(4.26, 0.0001)
        Expect(Math.Evaluate("sqrt(16) == 4")):ToBe(1)
        Expect(Math.Evaluate("pi")):ToBeCloseTo(math.pi, 0.000001)

        local damage = assert(Math.Compile("base * (1 + strength / 100) - armor"))
        Expect(damage:GetVariables()):ToEqual({ "armor", "base", "strength" })
        Expect(damage:Evaluate({ base = 50, strength = 20, armor = 10 })):ToBe(50)
        Expect(damage({ base = 100, strength = 0, armor = 0 })):ToBe(100)
        Expect(select(2, damage:Evaluate({ base = 1 }))):ToContain("'strength' has no value")

        for _, broken in ipairs({ "1 +", "(1", "foo(1)", "max()", "1 $ 2", "" }) do
            local value, message = Math.Evaluate(broken)
            Expect(value):ToBeNil()
            Expect(message):ToContain("invalid expression")
        end
        Expect(select(2, Math.Evaluate(string.rep("(", 500) .. "1" .. string.rep(")", 500)))):ToContain("nested too deeply")
    end)
end)

Test.Describe("Geometry", function()
    local Geometry = Quark.Geometry
    local Random = Quark.Random

    Test.It("checks and samples 2D shapes", function()
        local generator = Random.New(5)
        local circle = Geometry.Circle({ X = 10, Y = 10 }, 5)
        Expect(circle:Contains({ X = 12, Y = 12, Z = 500 })):ToBe(true)
        Expect(circle:Contains({ X = 16, Y = 10 })):ToBe(false)
        Expect(circle:GetArea()):ToBeCloseTo(78.5398, 0.001)
        Expect(circle:GetVolume()):ToBeNil()
        Expect(circle:Is3D()):ToBe(false)

        local rectangle = Geometry.Rectangle({ X = 0, Y = 0 }, { X = 10, Y = 2 }, 90)
        Expect(rectangle:Contains({ X = 1, Y = 9 })):ToBe(true)
        Expect(rectangle:Contains({ X = 9, Y = 1 })):ToBe(false)

        local shape = Geometry.Polygon({ { X = 0, Y = 0 }, { X = 10, Y = 0 }, { X = 10, Y = 10 }, { X = 5, Y = 10 }, { X = 5, Y = 5 }, { X = 0, Y = 5 } })
        Expect(shape:GetArea()):ToBe(75)
        Expect(shape:Contains({ X = 2, Y = 8 })):ToBe(false)
        for _, point in ipairs(shape:RandomPoints(500, generator)) do
            Expect(shape:Contains(point)):ToBe(true)
        end
        for _, item in ipairs({ circle, rectangle }) do
            for _, point in ipairs(item:RandomPoints(200, generator)) do
                Expect(item:Contains(point)):ToBe(true)
            end
        end
        local closest = shape:ClosestPoint({ X = 2, Y = 8 })
        Expect(math.sqrt((closest.X - 2) ^ 2 + (closest.Y - 8) ^ 2)):ToBeCloseTo(3, 0.0001)
        near(shape:ClosestPoint({ X = 12, Y = 2 }), 10, 2)
        Expect(shape:Distance({ X = 2, Y = 8 })):ToBeCloseTo(3, 0.0001)
        Expect(shape:Distance({ X = 1, Y = 1 })):ToBe(0)
        local minimum, maximum = shape:GetBounds()
        near(minimum, 0, 0)
        near(maximum, 10, 10)
        Expect(shape:GetType()):ToBe("Polygon")
    end)

    Test.It("checks and samples 3D shapes", function()
        local generator = Random.New(9)
        local sphere = Geometry.Sphere({ X = 0, Y = 0, Z = 0 }, 10)
        local box = Geometry.Box({ X = 100, Y = 0, Z = 0 }, { X = 10, Y = 5, Z = 5 }, { Pitch = 0, Yaw = 90, Roll = 0 })
        local cylinder = Geometry.Cylinder({ X = 0, Y = 0, Z = 50 }, 5, 20)
        local capsule = Geometry.Capsule({ X = 0, Y = 0, Z = 0 }, { X = 0, Y = 0, Z = 100 }, 10)
        local zone = Geometry.Prism({ { X = 0, Y = 0 }, { X = 100, Y = 0 }, { X = 0, Y = 100 } }, 0, 50)

        Expect(box:Contains({ X = 100, Y = 9, Z = 0 })):ToBe(true)
        Expect(box:Contains({ X = 109, Y = 0, Z = 0 })):ToBe(false)
        Expect(cylinder:Contains({ X = 3, Y = 3, Z = 59 })):ToBe(true)
        Expect(cylinder:Contains({ X = 0, Y = 0, Z = 61 })):ToBe(false)
        Expect(capsule:Contains({ X = 0, Y = 0, Z = 105 })):ToBe(true)
        Expect(zone:Contains({ X = 10, Y = 10, Z = 25 })):ToBe(true)
        Expect(zone:Contains({ X = 10, Y = 10, Z = 60 })):ToBe(false)
        Expect(zone:Contains({ X = 90, Y = 90, Z = 25 })):ToBe(false)
        Expect(sphere:GetVolume()):ToBeCloseTo(4188.79, 0.01)
        Expect(box:GetVolume()):ToBe(2000)
        Expect(zone:GetVolume()):ToBe(250000)

        for _, item in ipairs({ sphere, box, cylinder, capsule, zone }) do
            Expect(item:Is3D()):ToBe(true)
            for _, point in ipairs(item:RandomPoints(300, generator)) do
                Expect(item:Contains(point)):ToBe(true)
            end
        end
        near(sphere:ClosestPoint({ X = 20, Y = 0, Z = 0 }), 10, 0, 0)
        near(box:ClosestPoint({ X = 100, Y = 20, Z = 0 }), 100, 10, 0)
        Expect(capsule:Distance({ X = 30, Y = 0, Z = 50 })):ToBeCloseTo(20, 0.0001)
        near(zone:ClosestPoint({ X = 10, Y = 10, Z = 80 }), 10, 10, 50)
        local low, high = box:GetBounds()
        near(low, 95, -10, -5)
        near(high, 105, 10, 5)
    end)

    Test.It("combines shapes and stays deterministic", function()
        local spawn = Geometry.Union({ Geometry.Sphere({ X = 0, Y = 0, Z = 0 }, 1), Geometry.Sphere({ X = 100, Y = 0, Z = 0 }, 1) })
        local first = spawn:RandomPoints(20, Random.New(3))
        Expect(spawn:RandomPoints(20, Random.New(3))):ToEqual(first)
        local counts = { 0, 0 }
        for _, point in ipairs(spawn:RandomPoints(1000, Random.New(4))) do
            counts[point.X > 50 and 2 or 1] = counts[point.X > 50 and 2 or 1] + 1
        end
        Expect(counts[1] > 400 and counts[2] > 400):ToBe(true)
        Expect(spawn:Contains({ X = 100, Y = 0.5, Z = 0 })):ToBe(true)
        Expect(spawn:Distance({ X = 50, Y = 0, Z = 0 })):ToBeCloseTo(49, 0.0001)
        Expect(spawn:RandomPoint()):ToBeType("table")
        Expect(function() Geometry.Union({ spawn, Geometry.Circle({ X = 0, Y = 0 }, 1) }) end):ToThrow("cannot mix")
        Expect(function() Geometry.Sphere({ X = 0, Y = 0, Z = 0 }, 0) end):ToThrow("greater than 0")
        Expect(Geometry.IsShape(spawn)):ToBe(true)
    end)

    Test.It("works with polygons, segments and point sets", function()
        local square = { { X = 0, Y = 0 }, { X = 4, Y = 0 }, { X = 4, Y = 4 }, { X = 0, Y = 4 } }
        Expect(Geometry.PointInPolygon({ X = 2, Y = 2 }, square)):ToBe(true)
        Expect(Geometry.PointInPolygon({ X = 5, Y = 2 }, square)):ToBe(false)
        Expect(Geometry.PolygonArea(square)):ToBe(16)
        Expect(Geometry.PolygonArea({ square[1], square[4], square[3], square[2] }, true)):ToBe(-16)
        near(Geometry.PolygonCentroid(square), 2, 2)
        local hull = Geometry.ConvexHull({ { 0, 0 }, { 2, 1 }, { 4, 0 }, { 2, 4 }, { 1, 1 } })
        Expect(#hull):ToBe(3)
        near(Geometry.SegmentIntersection({ 0, 0 }, { 4, 4 }, { 0, 4 }, { 4, 0 }), 2, 2)
        Expect(Geometry.SegmentIntersection({ 0, 0 }, { 1, 1 }, { 2, 0 }, { 3, 1 })):ToBeNil()
        near(Geometry.ClosestPointOnSegment({ X = 5, Y = 5, Z = 0 }, { X = 0, Y = 0, Z = 0 }, { X = 10, Y = 0, Z = 0 }), 5, 0, 0)
        Expect(Geometry.DistanceToSegment({ X = 5, Y = 5, Z = 0 }, { X = 0, Y = 0, Z = 0 }, { X = 10, Y = 0, Z = 0 })):ToBe(5)
        Expect(#Geometry.Triangulate(square)):ToBe(2)
        Expect(#Geometry.Delaunay({ { 0, 0 }, { 1, 0 }, { 0, 1 }, { 1, 1 } })):ToBe(2)
        local simplified = Geometry.SimplifyPath({ { 0, 0, 0 }, { 1, 0.01, 0 }, { 2, 0, 0 }, { 3, 5, 0 } }, 0.1)
        Expect(#simplified):ToBe(3)
        local low, high = Geometry.BoundingBox({ { 1, 5, 3 }, { -2, 0, 8 } }, { Padding = 1 })
        near(low, -3, -1, 2)
        near(high, 2, 6, 9)
    end)
end)

Test.Describe("BigInt", function()
    local BigInt = Quark.BigInt

    Test.It("calculates with large integers", function()
        Expect(tostring(BigInt.Factorial(25))):ToBe("15511210043330985984000000")
        local big = BigInt.New("123456789012345678901234567890")
        Expect(tostring(big * 2 + 1)):ToBe("246913578024691357802469135781")
        Expect(tostring(big // 1000)):ToBe("123456789012345678901234567")
        Expect(tostring(BigInt.New(-7) % 3)):ToBe("2")
        Expect(tostring(BigInt.New(2) ^ 100)):ToBe("1267650600228229401496703205376")
        Expect(tostring(BigInt.New(4):ModPow(13, 497))):ToBe("445")
        Expect(tostring(BigInt.New(48):Gcd(36))):ToBe("12")
        Expect(BigInt.New("0xff"):ToNumber()):ToBe(255)
        Expect(BigInt.New(255):ToString(16)):ToBe("ff")
        Expect(BigInt.New(10) < BigInt.New(11)):ToBe(true)
        Expect(big > 5):ToBe(true)
        Expect(BigInt.New(5) == BigInt.New(5)):ToBe(true)
        Expect(BigInt.New(5):Equals(5)):ToBe(true)
        Expect("value: " .. BigInt.New(42)):ToBe("value: 42")
        Expect(BigInt.New(99):FitsInteger()):ToBe(true)
        Expect(BigInt.New("1" .. string.rep("0", 30)):FitsInteger()):ToBe(false)
        Expect(function() return BigInt.New(1) // 0 end):ToThrow("division by zero")
        Expect(BigInt.Parse("12x")):ToBeNil()
    end)
end)

Test.Describe("Decimal", function()
    local Decimal = Quark.Decimal

    Test.It("calculates exactly", function()
        Expect(Decimal.New("0.1") + Decimal.New("0.2") == Decimal.New("0.3")):ToBe(true)
        Expect(tostring(Decimal.New(0.1) + 0.2)):ToBe("0.3")
        Expect(tostring(Decimal.New("19.99") * 3)):ToBe("59.97")
        Expect(tostring(Decimal.New(10) / 4)):ToBe("2.5")
        Expect(tostring(Decimal.New("2.345"):Round(2))):ToBe("2.34")
        Expect(tostring(Decimal.New("2.345"):Round(2, "HalfUp"))):ToBe("2.35")
        Expect(tostring(Decimal.New("-2.5"):Round(0, "Floor"))):ToBe("-3")
        Expect(Decimal.New("1234567.891"):Format({ Decimals = 2 })):ToBe("1,234,567.89")
        Expect(Decimal.New("-1234.5"):Format({ Decimals = 2, Separator = ".", Point = "," })):ToBe("-1.234,50")
        local shares = Decimal.New("10.00"):Split(3)
        Expect({ tostring(shares[1]), tostring(shares[2]), tostring(shares[3]) }):ToEqual({ "3.34", "3.33", "3.33" })
        Expect(tostring(Decimal.Sum({ "1.10", 2, Decimal.New("0.05") }))):ToBe("3.15")
        Expect(Decimal.New("1.5"):ToNumber()):ToBe(1.5)
        Expect(Decimal.New(3) > 2):ToBe(true)
        Expect(function() return Decimal.New(1) / 0 end):ToThrow("divides by zero")
        Expect(Decimal.Parse("abc")):ToBeNil()
    end)
end)
