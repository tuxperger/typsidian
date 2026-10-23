// typsidian-embed v1
// Встраивание заметок Typsidian (фича 031). Файл создаёт приложение;
// не правьте его — при смене версии он будет заменён.
// Использование: #import "/.typsidian/lib/embed.typ": embed
//                #embed("Методика"), #embed("Методика#Выборка"), #embed("Методика#<метка>")

// Подпись на языке документа: по-русски для `ru`, иначе по-английски.
#let _t(ru, en) = if text.lang == "ru" { ru } else { en }

// Плашка на месте встраивания: ошибка (красная) или подсказка (серая).
#let _plate(title, body, hint: false) = block(
  width: 100%,
  breakable: false,
  inset: (x: 8pt, y: 6pt),
  fill: if hint { rgb("#eef0f2") } else { rgb("#fbecea") },
  stroke: (left: 2.5pt + if hint { rgb("#7a8691") } else { rgb("#c0392b") }),
  text(size: 0.85em, fill: if hint { rgb("#2d343b") } else { rgb("#5a1f19") })[
    #strong(title) \
    #body
  ],
)

// Плашка по ответу приложения (contracts/core-embeds-api.md).
#let _failure(entry) = context {
  let target = raw(entry.target)
  let reason = entry.error
  if reason == "note" {
    _plate(
      _t("Встраивание не найдено", "Embed not found"),
      _t([Заметки #target нет в vault.], [There is no note #target in the vault.]),
    )
  } else if reason == "part" {
    let part = raw(entry.at("part", default: ""))
    _plate(
      _t("Встраивание не найдено", "Embed not found"),
      _t([В заметке #target нет раздела или метки #part.], [Note #target has no section or label #part.]),
    )
  } else if reason == "cycle" {
    let chain = entry.at("chain", default: ()).join(" → ")
    _plate(
      _t("Встраивание прервано: цикл", "Embed stopped: cycle"),
      _t([#target уже встроена выше по цепочке: #raw(chain).], [#target is already embedded above: #raw(chain).]),
    )
  } else if reason == "depth" {
    _plate(
      _t("Встраивание прервано: глубже 5 уровней", "Embed stopped: deeper than 5 levels"),
      _t([Цель #target не показана.], [Target #target is not shown.]),
    )
  } else if reason == "board" {
    _plate(
      _t("Доска встраивается вставкой доски", "Boards are embedded with Insert board"),
      _t([#target — «Вставить доску» покажет её изображение.], [#target — “Insert board” shows its image.]),
      hint: true,
    )
  } else {
    _plate(_t("Встраивание не найдено", "Embed not found"), target)
  }
}

// Содержимое встраивания: приложение кладёт его в сборку и отвечает через
// `sys.inputs`; без приложения — плашка с целью.
#let embed(target) = {
  let payload = sys.inputs.at("typsidian-embed", default: none)
  let entry = if payload == none { none } else { json(bytes(payload)).at(target, default: none) }
  if payload == none or entry == none {
    context _plate(
      _t("Встраивание: ", "Embed: ") + target,
      _t([Показывается в Typsidian.], [Shown in Typsidian.]),
      hint: true,
    )
  } else if "path" in entry {
    include entry.path
  } else {
    _failure(entry)
  }
}
