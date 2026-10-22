# Locale

```lua
local Locale = Quark.Locale

local texts = assert(Locale.New({
    Fallback = "en",
    Resources = { en = ENGLISH, de = GERMAN },
}))

Chat.SendMessage(player, texts:Format(player:GetValue("language") or "en", "kills", { count = 3 }))
```

Translations with [Project Fluent](https://projectfluent.org/), the localization system of Firefox. Fluent handles plurals, gender and other grammar per language inside the translation files, so scripts only pass values and translators decide how each language says it.

## Fluent files

```ftl
welcome = Welcome, { $name }!

kills = { $count ->
    [one] { $count } kill
   *[other] { $count } kills
}

menu = Menu
    .title = Main menu
```

```ftl
welcome = Willkommen, { $name }!

kills = { $count ->
    [one] { $count } Abschuss
   *[other] { $count } Abschüsse
}
```

Numbers select the plural category of the language (`zero`, `one`, `two`, `few`, `many`, `other`), Russian for example needs `one`, `few` and `many`. The [Fluent syntax guide](https://projectfluent.org/fluent/guide/) covers selectors, terms, attributes and more. nanos world packages can only load `.lua` files, so keep each language in a Lua file that stores the Fluent text in a long string (`ENGLISH = [[ ... ]]`) and load it with `Package.Require`, or fetch the texts from a database or over HTTP.

## Locales

`Locale.New(options?)` returns a set of translations, or `nil, message` when a resource has syntax errors.

| Option | |
| --- | --- |
| `Resources` | table of language tags (`en`, `de`, `pt-BR`) and Fluent sources |
| `Fallback` | language used when a message is missing in the requested one |
| `Isolating` | wrap arguments in Unicode isolation marks for mixed left to right and right to left text (`false`) |

| Method | |
| --- | --- |
| `:Format(language, id, arguments?)` | `text, found, problems` |
| `:Bind(language)` | a function `(id, arguments?)` formatting in that language |
| `:Add(language, source)` | adds messages, replacing existing ones with the same id. Returns `true` or `nil, message` |
| `:Has(language, id)` | whether the message exists in the language or its fallbacks |
| `:Negotiate(requested)` | the best available language for a tag, a list of tags or an `Accept-Language` header |
| `:GetLocales()`, `:GetFallback()` | |

`Format` looks for the message in the requested language (`de-AT`), its base language (`de`) and the fallback, in that order. Attributes are addressed as `"menu.title"`. When no language has the message it returns the id and `false`. `problems` describes missing arguments and similar issues, the text is still returned with the missing parts marked, like `Welcome, {$name}!`.

```lua
local function Translate(player, id, arguments)
    return texts:Format(player:GetValue("language") or "en", id, arguments)
end
```

## Plural rules and negotiation

`Locale.PluralCategory(language, number, type?)` returns the plural category of a number, for the `Cardinal` (default, "3 apples") or `Ordinal` type ("3rd place"): `Locale.PluralCategory("en", 3, "Ordinal")` is `"few"`, as in "3rd".

`Locale.Negotiate(requested, available, fallback?)` picks the best match from a list of language tags without a locale object, for example to choose a language from the `Accept-Language` header of an HTTP request.
