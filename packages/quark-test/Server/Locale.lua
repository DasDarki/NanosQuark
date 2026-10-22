local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Locale", function()
    local Locale = Quark.Locale

    local function create()
        return assert(Locale.New({
            Fallback = "en",
            Resources = {
                en = [[
welcome = Welcome, { $name }!
kills = { $count ->
    [one] { $count } kill
   *[other] { $count } kills
}
menu = Menu
    .title = Main menu
only-english = Only in English
]],
                de = [[
welcome = Willkommen, { $name }!
kills = { $count ->
    [one] { $count } Abschuss
   *[other] { $count } Abschüsse
}
]],
                ru = [[
apples = { $count ->
    [one] { $count } яблоко
    [few] { $count } яблока
   *[many] { $count } яблок
}
]],
            },
        }))
    end

    Test.It("formats messages with arguments and plurals", function()
        local locale = create()
        Expect({ locale:Format("de", "welcome", { name = "Ada" }) }):ToEqual({ "Willkommen, Ada!", true })
        Expect(locale:Format("en", "kills", { count = 1 })):ToBe("1 kill")
        Expect(locale:Format("en", "kills", { count = 5 })):ToBe("5 kills")
        Expect(locale:Format("de", "kills", { count = 1 })):ToBe("1 Abschuss")
        Expect(locale:Format("ru", "apples", { count = 3 })):ToBe("3 яблока")
        Expect(locale:Format("ru", "apples", { count = 11 })):ToBe("11 яблок")
        Expect(locale:Format("ru", "apples", { count = 21 })):ToBe("21 яблоко")
        Expect(locale:Format("en", "menu.title")):ToBe("Main menu")
    end)

    Test.It("falls back to parent and fallback locales", function()
        local locale = create()
        Expect(locale:Format("de-AT", "welcome", { name = "Max" })):ToBe("Willkommen, Max!")
        Expect(locale:Format("de", "only-english")):ToBe("Only in English")
        Expect(locale:Format("fr", "menu")):ToBe("Menu")
        Expect({ locale:Format("de", "missing") }):ToEqual({ "missing", false })
        Expect(locale:Has("de", "only-english")):ToBe(true)
        Expect(locale:Has("de", "missing")):ToBe(false)
    end)

    Test.It("reports missing arguments", function()
        local text, found, problems = create():Format("en", "welcome")
        Expect(text):ToBe("Welcome, {$name}!")
        Expect(found):ToBe(true)
        Expect(problems):ToContain("name")
    end)

    Test.It("adds resources and negotiates locales", function()
        local locale = create()
        Expect(locale:Add("de", "menu = Menü")):ToBe(true)
        Expect(locale:Format("de", "menu")):ToBe("Menü")
        Expect(locale:Add("de", "welcome = Hallo { $name }")):ToBe(true)
        Expect(locale:Format("de", "welcome", { name = "Ada" })):ToBe("Hallo Ada")
        local result, message = locale:Add("en", "broken = { $")
        Expect(result):ToBeNil()
        Expect(message):ToContain("invalid Fluent syntax")
        Expect(function() locale:Add("not a tag!", "a = b") end):ToThrow("language tag")

        Expect(locale:GetLocales()):ToEqual({ "de", "en", "ru" })
        Expect(locale:GetFallback()):ToBe("en")
        Expect(locale:Negotiate("de-CH,de;q=0.9,en;q=0.8")):ToBe("de")
        Expect(locale:Negotiate({ "fr", "ru" })):ToBe("ru")
        Expect(locale:Negotiate("ja")):ToBe("en")
        Expect(Locale.Negotiate("pt-BR", { "pt", "es" })):ToBe("pt")
        Expect(Locale.Negotiate("ja", { "pt" })):ToBeNil()

        local german = locale:Bind("de")
        Expect(german("kills", { count = 2 })):ToBe("2 Abschüsse")
    end)

    Test.It("selects plural categories", function()
        Expect(Locale.PluralCategory("en", 1)):ToBe("one")
        Expect(Locale.PluralCategory("en", 2)):ToBe("other")
        Expect(Locale.PluralCategory("pl", 3)):ToBe("few")
        Expect(Locale.PluralCategory("ar", 0)):ToBe("zero")
        Expect(Locale.PluralCategory("en", 2, "Ordinal")):ToBe("two")
        Expect(Locale.PluralCategory("en-US", 3, "Ordinal")):ToBe("few")
        Expect(function() Locale.PluralCategory("en", 1, "Weird") end):ToThrow("unknown plural type")
    end)
end)
