#set page(paper: "presentation-16-9", margin: 2cm)
#set text(size: 24pt, lang: "ru")

#let slide(number, body, note: none) = {
  body
  [#metadata((t: "LogicalSlide", v: number)) <pdfpc>]
  if note != none { [#metadata((t: "Note", v: note)) <pdfpc>] }
}

#slide(1, note: "Представиться, назвать тему и срок защиты.")[
  #align(center + horizon)[
    #text(size: 40pt, weight: "bold")[Опрос о чтении]
    #v(1em)
    Промежуточные результаты
  ]
]
#pagebreak()
#slide(2, note: "Подчеркнуть, что пилот уже собран.")[
  = Методика
  - Анкета из 24 вопросов
  - Пилот на 30 участниках
  - Основной этап: $n approx 384$
]
#pagebreak()
#slide(3)[
  = Что дальше
  + Пересчитать выборку
  + Запустить основной этап
  + Сдать черновик главы
]
