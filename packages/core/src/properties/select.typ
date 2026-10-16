// typsidian-select v1
// Библиотека блоков выборки Typsidian (фича 020). Файл создаёт приложение;
// не правьте его — при смене версии он будет заменён.
// Использование: #import "/.typsidian/lib/select.typ": select

// Словари с ключами по алфавиту: из такого вида строится ключ блока, и ядро
// строит его точно так же.
#let _canon(v) = {
  if type(v) == dictionary {
    v.pairs()
      .sorted(key: p => p.at(0))
      .fold((:), (acc, p) => {
        let out = acc
        out.insert(p.at(0), _canon(p.at(1)))
        out
      })
  } else if type(v) == array {
    v.map(_canon)
  } else {
    v
  }
}

// Подпись на языке документа: по-русски для `ru`, иначе по-английски.
#let _t(ru, en) = if text.lang == "ru" { ru } else { en }

// Условие словами — для замещающего текста и пояснения о пустой выборке.
#let _describe(c) = {
  if "tag" in c {
    [#_t("тег", "tag") #raw(c.tag)]
  } else if "folder" in c {
    [#_t("папка", "folder") #raw(c.folder)]
  } else if "exists" in c {
    [#raw(c.key) #_t("задано", "is set")]
  } else if "op" in c {
    let shown = if "date" in c {
      c.date
    } else if type(c.value) == str {
      c.value
    } else {
      repr(c.value)
    }
    [#raw(c.key) #c.op #raw(shown)]
  } else if "modified" in c or "created" in c {
    let name = if "modified" in c { "modified" } else { "created" }
    let range = c.at(name)
    let ru-name = if name == "modified" { "изменена" } else { "создана" }
    if type(range) == int {
      [#_t(ru-name + " за " + str(range) + " дн.", name + " in the last " + str(range) + " days")]
    } else {
      [#_t(ru-name + " с ", name + " from ") #raw(range.at(0)) #_t("по", "to") #raw(range.at(1))]
    }
  } else {
    []
  }
}

#let _conditions(where) = {
  if where.len() > 0 {
    [: ]
    where.map(_describe).join([, ])
  }
}

#let _frame(body) = block(
  width: 100%,
  inset: 8pt,
  radius: 3pt,
  stroke: (paint: luma(160), dash: "dashed"),
  text(fill: luma(90), size: 0.9em, body),
)

#let _cell(c) = if c.l != none { link(c.l, c.s) } else { c.s }

#let _head(name) = {
  let parts = name.clusters()
  strong(upper(parts.first()) + parts.slice(1).sum(default: ""))
}

#let select(
  where: (),
  sort: none,
  columns: ("название",),
  limit: 50,
  view: "table",
) = context {
  let key = json.encode(
    _canon((where: where, sort: sort, columns: columns, limit: limit, view: view)),
    pretty: false,
  )
  let payload = sys.inputs.at("typsidian-select", default: none)
  let found = if payload == none { none } else { json(bytes(payload)).at(key, default: none) }
  if found == none {
    _frame[#_t("Выборка показывается в Typsidian", "The selection is shown in Typsidian")#_conditions(where)]
  } else if found.rows.len() == 0 {
    _frame[#_t("Нет заметок по выборке", "No notes match the selection")#_conditions(where)]
  } else {
    if view == "list" {
      list(..found.rows.map(r => r.cells.map(_cell).join([ — ])))
    } else {
      table(
        columns: found.columns.len(),
        ..found.columns.map(_head),
        ..found.rows.map(r => r.cells.map(_cell)).flatten(),
      )
    }
    if found.truncated {
      text(size: 0.85em, fill: luma(110))[#_t(
        "Показаны первые " + str(found.rows.len()) + " из " + str(found.total),
        "Showing the first " + str(found.rows.len()) + " of " + str(found.total),
      )]
    }
  }
}
