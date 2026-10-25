---@meta

---@alias QuarkVector Vector|{ X: number, Y: number, Z: number? }|number[]
---@alias QuarkRotator Rotator|{ Pitch: number, Yaw: number, Roll: number? }|number[]
---@alias QuarkRotation QuarkRotator|QuarkQuaternion
---@alias QuarkNumberOrVector number|QuarkVector

---Unreal style quaternion.
---@class Vector
---@field X number
---@field Y number
---@field Z number

---@class Vector2D
---@field X number
---@field Y number

---@class Rotator
---@field Pitch number
---@field Yaw number
---@field Roll number

---@class QuarkQuaternion
---@field X number
---@field Y number
---@field Z number
---@field W number
---@operator mul(QuarkQuaternion): QuarkQuaternion
---@operator mul(QuarkVector): Vector
local Quaternion = {}

---@param other QuarkRotation
---@return QuarkQuaternion
function Quaternion:Multiply(other) end

---@param vector QuarkVector
---@return Vector
function Quaternion:RotateVector(vector) end

---@param vector QuarkVector
---@return Vector
function Quaternion:UnrotateVector(vector) end

---@return QuarkQuaternion
function Quaternion:Inverse() end

---@return QuarkQuaternion
function Quaternion:Normalize() end

---@param other QuarkRotation
---@return number
function Quaternion:Dot(other) end

---@param other QuarkRotation
---@return number
function Quaternion:AngleTo(other) end

---@param other QuarkRotation
---@param t number
---@return QuarkQuaternion
function Quaternion:Slerp(other, t) end

---@return Rotator
function Quaternion:ToRotator() end

---@return Vector axis
---@return number degrees
function Quaternion:ToAxisAngle() end

---@return Vector
function Quaternion:GetForward() end

---@return Vector
function Quaternion:GetRight() end

---@return Vector
function Quaternion:GetUp() end

---Compiled formula, can be called like a function.
---@class QuarkExpression
---@operator call(table<string, number|boolean>?): number?, string?
local Expression = {}

---@param variables? table<string, number|boolean>
---@return number?
---@return string? message
function Expression:Evaluate(variables) end

---@return string[]
function Expression:GetVariables() end

---@return string
function Expression:GetSource() end

---@class QuarkMathModule
---@field Pi number
---@field Tau number
local Math = {}

---@param value number
---@param minimum number
---@param maximum number
---@return number
function Math.Clamp(value, minimum, maximum) end

---@param from QuarkNumberOrVector
---@param to QuarkNumberOrVector
---@param t number
---@return number|Vector
function Math.Lerp(from, to, t) end

---@param from number
---@param to number
---@param value number
---@return number
function Math.InverseLerp(from, to, value) end

---@param value number
---@param from_minimum number
---@param from_maximum number
---@param to_minimum number
---@param to_maximum number
---@param clamp? boolean
---@return number
function Math.Remap(value, from_minimum, from_maximum, to_minimum, to_maximum, clamp) end

---@param edge0 number
---@param edge1 number
---@param value number
---@return number
function Math.SmoothStep(edge0, edge1, value) end

---@param current number
---@param target number
---@param step number
---@return number
function Math.MoveTowards(current, target, step) end

---@param value number
---@param decimals? integer
---@return number
function Math.Round(value, decimals) end

---@param value number
---@return integer
function Math.Sign(value) end

---@param value number
---@param minimum number
---@param maximum number
---@return number
function Math.Wrap(value, minimum, maximum) end

---@param angle number
---@return number
function Math.NormalizeAngle(angle) end

---@param from number
---@param to number
---@return number
function Math.DeltaAngle(from, to) end

---@param from number
---@param to number
---@param t number
---@return number
function Math.LerpAngle(from, to, t) end

---@param first number
---@param second number
---@param tolerance? number
---@return boolean
function Math.IsNearlyEqual(first, second, tolerance) end

---@param name string
---@param t number
---@return number
function Math.Ease(name, t) end

---@return string[]
function Math.GetEasings() end

---@param from QuarkNumberOrVector
---@param to QuarkNumberOrVector
---@param t number
---@param easing? string
---@return number|Vector
function Math.Tween(from, to, t, easing) end

---@param points QuarkNumberOrVector[]
---@param t number
---@return number|Vector
function Math.Bezier(points, t) end

---@param points QuarkNumberOrVector[]
---@param t number
---@param options? { Closed?: boolean, Alpha?: number }
---@return number|Vector
function Math.Spline(points, t, options) end

---@param points QuarkNumberOrVector[]
---@param count integer
---@param options? { Closed?: boolean, Alpha?: number }
---@return (number|Vector)[]
function Math.SampleSpline(points, count, options) end

---@param x? number
---@param y? number
---@param z? number
---@param w? number
---@return QuarkQuaternion
function Math.Quaternion(x, y, z, w) end

---@param rotator QuarkRotator
---@return QuarkQuaternion
function Math.QuaternionFromRotator(rotator) end

---@param axis QuarkVector
---@param degrees number
---@return QuarkQuaternion
function Math.QuaternionFromAxisAngle(axis, degrees) end

---@param from QuarkVector
---@param to QuarkVector
---@return QuarkQuaternion
function Math.QuaternionBetween(from, to) end

---@param value any
---@return boolean
function Math.IsQuaternion(value) end

---@param vector QuarkVector
---@param rotation QuarkRotation
---@return Vector
function Math.RotateVector(vector, rotation) end

---@param vector QuarkVector
---@param rotation QuarkRotation
---@return Vector
function Math.UnrotateVector(vector, rotation) end

---@param rotation QuarkRotation
---@return Vector forward
---@return Vector right
---@return Vector up
function Math.GetDirections(rotation) end

---@param from QuarkVector
---@param to QuarkVector
---@return Rotator
function Math.LookAt(from, to) end

---@param from QuarkRotator
---@param to QuarkRotator
---@param t number
---@return Rotator
function Math.SlerpRotator(from, to, t) end

---@param first QuarkRotator
---@param second QuarkRotator
---@return Rotator
function Math.CombineRotators(first, second) end

---@param rotator QuarkRotator
---@return Rotator
function Math.NormalizeRotator(rotator) end

---@param values number[]|QuarkBufferView
---@return number
function Math.Sum(values) end

---@param values number[]|QuarkBufferView
---@return number?
function Math.Mean(values) end

---@param values number[]|QuarkBufferView
---@return number?
function Math.Median(values) end

---@param values number[]|QuarkBufferView
---@return number?
function Math.Min(values) end

---@param values number[]|QuarkBufferView
---@return number?
function Math.Max(values) end

---@param values number[]|QuarkBufferView
---@param percent number
---@return number?
function Math.Percentile(values, percent) end

---@param values number[]|QuarkBufferView
---@param sample? boolean
---@return number?
function Math.Variance(values, sample) end

---@param values number[]|QuarkBufferView
---@param sample? boolean
---@return number?
function Math.StandardDeviation(values, sample) end

---@param values number[]|QuarkBufferView
---@return { Count: integer, Sum: number?, Mean: number?, Min: number?, Max: number?, Median: number?, P90: number?, P99: number?, StandardDeviation: number? }
function Math.Summarize(values) end

---@param values number[]|QuarkBufferView
---@param bins integer
---@param minimum? number
---@param maximum? number
---@return integer[]
function Math.Histogram(values, bins, minimum, maximum) end

---@param formula string
---@param variables? table<string, number|boolean>
---@return number?
---@return string? message
function Math.Evaluate(formula, variables) end

---@param formula string
---@return QuarkExpression?
---@return string? message
function Math.Compile(formula) end

---A 2D or 3D shape.
---@class QuarkShape
local Shape = {}

---@param point QuarkVector
---@return boolean
function Shape:Contains(point) end

---@param generator? QuarkRandomGenerator
---@return Vector|Vector2D
function Shape:RandomPoint(generator) end

---@param count integer
---@param generator? QuarkRandomGenerator
---@return (Vector|Vector2D)[]
function Shape:RandomPoints(count, generator) end

---@param point QuarkVector
---@return Vector|Vector2D
function Shape:ClosestPoint(point) end

---@param point QuarkVector
---@return number
function Shape:Distance(point) end

---@return Vector|Vector2D minimum
---@return Vector|Vector2D maximum
function Shape:GetBounds() end

---@return Vector|Vector2D
function Shape:GetCenter() end

---@return number?
function Shape:GetArea() end

---@return number?
function Shape:GetVolume() end

---@return "Circle"|"Rectangle"|"Polygon"|"Sphere"|"Box"|"Cylinder"|"Capsule"|"Prism"|"Union"
function Shape:GetType() end

---@return boolean
function Shape:Is3D() end

---@class QuarkGeometryModule
local Geometry = {}

---@param center QuarkVector
---@param radius number
---@return QuarkShape
function Geometry.Circle(center, radius) end

---@param center QuarkVector
---@param extent QuarkVector
---@param angle? number
---@return QuarkShape
function Geometry.Rectangle(center, extent, angle) end

---@param points QuarkVector[]
---@return QuarkShape
function Geometry.Polygon(points) end

---@param center QuarkVector
---@param radius number
---@return QuarkShape
function Geometry.Sphere(center, radius) end

---@param center QuarkVector
---@param extent QuarkVector
---@param rotation? QuarkRotation
---@return QuarkShape
function Geometry.Box(center, extent, rotation) end

---@param center QuarkVector
---@param radius number
---@param height number
---@return QuarkShape
function Geometry.Cylinder(center, radius, height) end

---@param start QuarkVector
---@param finish QuarkVector
---@param radius number
---@return QuarkShape
function Geometry.Capsule(start, finish, radius) end

---@param points QuarkVector[]
---@param minimum number
---@param maximum number
---@return QuarkShape
function Geometry.Prism(points, minimum, maximum) end

---@param shapes QuarkShape[]
---@return QuarkShape
function Geometry.Union(shapes) end

---@param value any
---@return boolean
function Geometry.IsShape(value) end

---@param point QuarkVector
---@param polygon QuarkVector[]
---@return boolean
function Geometry.PointInPolygon(point, polygon) end

---@param polygon QuarkVector[]
---@param signed? boolean
---@return number
function Geometry.PolygonArea(polygon, signed) end

---@param polygon QuarkVector[]
---@return Vector2D
function Geometry.PolygonCentroid(polygon) end

---@param points QuarkVector[]
---@return Vector2D[]
function Geometry.ConvexHull(points) end

---@param a1 QuarkVector
---@param a2 QuarkVector
---@param b1 QuarkVector
---@param b2 QuarkVector
---@return Vector2D?
function Geometry.SegmentIntersection(a1, a2, b1, b2) end

---@param point QuarkVector
---@param start QuarkVector
---@param finish QuarkVector
---@return Vector
function Geometry.ClosestPointOnSegment(point, start, finish) end

---@param point QuarkVector
---@param start QuarkVector
---@param finish QuarkVector
---@return number
function Geometry.DistanceToSegment(point, start, finish) end

---@param polygon QuarkVector[]
---@return integer[][]
function Geometry.Triangulate(polygon) end

---@param points QuarkVector[]
---@return integer[][]
function Geometry.Delaunay(points) end

---@param points QuarkVector[]
---@param tolerance number
---@return Vector[]
function Geometry.SimplifyPath(points, tolerance) end

---@param points QuarkVector[]
---@param options? { Padding?: number }
---@return Vector? minimum
---@return Vector? maximum
function Geometry.BoundingBox(points, options) end

---Integer of any size, supports arithmetic operators.
---@class QuarkBigInt
---@operator add(QuarkBigInt|integer|string): QuarkBigInt
---@operator sub(QuarkBigInt|integer|string): QuarkBigInt
---@operator mul(QuarkBigInt|integer|string): QuarkBigInt
---@operator div(QuarkBigInt|integer|string): QuarkBigInt
---@operator idiv(QuarkBigInt|integer|string): QuarkBigInt
---@operator mod(QuarkBigInt|integer|string): QuarkBigInt
---@operator pow(QuarkBigInt|integer|string): QuarkBigInt
---@operator unm: QuarkBigInt
---@operator concat(any): string
---@diagnostic disable-next-line: unknown-operator
---@operator lt(QuarkBigInt|integer|string): boolean
---@diagnostic disable-next-line: unknown-operator
---@operator le(QuarkBigInt|integer|string): boolean
local BigInt = {}

---@param base? integer
---@return string
function BigInt:ToString(base) end

---@return number
function BigInt:ToNumber() end

---@return boolean
function BigInt:FitsInteger() end

---@return QuarkBigInt
function BigInt:Abs() end

---@param exponent integer
---@return QuarkBigInt
function BigInt:Pow(exponent) end

---@param exponent integer|string|QuarkBigInt
---@param modulus integer|string|QuarkBigInt
---@return QuarkBigInt
function BigInt:ModPow(exponent, modulus) end

---@param other integer|string|QuarkBigInt
---@return QuarkBigInt
function BigInt:Gcd(other) end

---@param other integer|string|QuarkBigInt
---@return QuarkBigInt
function BigInt:Lcm(other) end

---@return QuarkBigInt
function BigInt:Sqrt() end

---@param other integer|string|QuarkBigInt
---@return QuarkBigInt quotient
---@return QuarkBigInt remainder
function BigInt:DivMod(other) end

---@return -1|0|1
function BigInt:GetSign() end

---@return boolean
function BigInt:IsZero() end

---@return boolean
function BigInt:IsEven() end

---@return integer
function BigInt:GetBits() end

---@param other any
---@return boolean
function BigInt:Equals(other) end

---@param other integer|string|QuarkBigInt
---@return -1|0|1
function BigInt:Compare(other) end

---@class QuarkBigIntModule
local BigIntModule = {}

---@param value integer|number|string|QuarkBigInt
---@return QuarkBigInt
function BigIntModule.New(value) end

---@param text string
---@return QuarkBigInt?
---@return string? message
function BigIntModule.Parse(text) end

---@param value any
---@return boolean
function BigIntModule.IsBigInt(value) end

---@param value integer
---@return QuarkBigInt
function BigIntModule.Factorial(value) end

---Exact decimal number, supports arithmetic operators.
---@class QuarkDecimal
---@operator add(QuarkDecimal|number|string): QuarkDecimal
---@operator sub(QuarkDecimal|number|string): QuarkDecimal
---@operator mul(QuarkDecimal|number|string): QuarkDecimal
---@operator div(QuarkDecimal|number|string): QuarkDecimal
---@operator mod(QuarkDecimal|number|string): QuarkDecimal
---@operator unm: QuarkDecimal
---@operator concat(any): string
---@diagnostic disable-next-line: unknown-operator
---@operator lt(QuarkDecimal|number|string): boolean
---@diagnostic disable-next-line: unknown-operator
---@operator le(QuarkDecimal|number|string): boolean
local Decimal = {}

---@return string
function Decimal:ToString() end

---@return number
function Decimal:ToNumber() end

---@param options? { Decimals?: integer, Rounding?: "HalfEven"|"HalfUp"|"HalfDown"|"Up"|"Down"|"Ceil"|"Floor", Separator?: string, Point?: string }
---@return string
function Decimal:Format(options) end

---@param decimals? integer
---@param mode? "HalfEven"|"HalfUp"|"HalfDown"|"Up"|"Down"|"Ceil"|"Floor"
---@return QuarkDecimal
function Decimal:Round(decimals, mode) end

---@param decimals? integer
---@return QuarkDecimal
function Decimal:Truncate(decimals) end

---@return QuarkDecimal
function Decimal:Floor() end

---@return QuarkDecimal
function Decimal:Ceil() end

---@return QuarkDecimal
function Decimal:Abs() end

---@return QuarkDecimal
function Decimal:Sqrt() end

---@param exponent integer
---@return QuarkDecimal
function Decimal:Pow(exponent) end

---@param parts integer
---@return QuarkDecimal[]
function Decimal:Split(parts) end

---@return integer
function Decimal:GetScale() end

---@return boolean
function Decimal:IsZero() end

---@return boolean
function Decimal:IsNegative() end

---@param other any
---@return boolean
function Decimal:Equals(other) end

---@param other number|string|QuarkDecimal
---@return -1|0|1
function Decimal:Compare(other) end

---@class QuarkDecimalModule
local DecimalModule = {}

---@param value number|string|QuarkDecimal
---@return QuarkDecimal
function DecimalModule.New(value) end

---@param text string
---@return QuarkDecimal?
---@return string? message
function DecimalModule.Parse(text) end

---@param value any
---@return boolean
function DecimalModule.IsDecimal(value) end

---@param values (number|string|QuarkDecimal)[]
---@return QuarkDecimal
function DecimalModule.Sum(values) end
