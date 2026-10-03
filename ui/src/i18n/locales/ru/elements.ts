import type { Translation } from "../define.js";
import type { elements as en } from "../en/elements.js";

export const elements: Translation<typeof en> = {
  "elements.category.struct": "Структура",
  "elements.category.list": "Списки",
  "elements.category.math": "Математика",
  "elements.category.block": "Блоки",
  "elements.category.layout": "Макет",
  "elements.category.draw": "Графика",
  "elements.category.style": "Стили",
  "elements.category.data": "Данные",
  "elements.category.link": "Ссылки",
  "elements.category.mark": "Разметка",

  "elements.heading-1.name": "Заголовок 1",
  "elements.heading-1.summary": "Раздел верхнего уровня",
  "elements.heading-1.template": "= ${Заголовок}",

  "elements.heading-2.name": "Заголовок 2",
  "elements.heading-2.summary": "Подраздел",
  "elements.heading-2.template": "== ${Подзаголовок}",

  "elements.heading-3.name": "Заголовок 3",
  "elements.heading-3.summary": "Пункт внутри подраздела",
  "elements.heading-3.template": "=== ${Пункт}",

  "elements.heading-4.name": "Заголовок 4",
  "elements.heading-4.summary": "Подпункт",
  "elements.heading-4.template": "==== ${Подпункт}",

  "elements.title.name": "Заглавие документа",
  "elements.title.summary": "Название документа, один раз на документ",
  "elements.title.template": "#set document(title: [${Название}])\n#title()",

  "elements.outline.name": "Оглавление",
  "elements.outline.summary": "Содержание по заголовкам до указанной глубины",
  "elements.outline.template": "#outline(title: [${Содержание}], depth: ${2})",

  "elements.divider.name": "Тематический разрыв",
  "elements.divider.summary": "Черта между частями текста",
  "elements.divider.template": "#divider()",

  "elements.pagebreak.name": "Разрыв страницы",
  "elements.pagebreak.summary": "Дальше — со следующей страницы",
  "elements.pagebreak.template": "#pagebreak()",

  "elements.parbreak.name": "Разрыв абзаца",
  "elements.parbreak.summary": "Конец абзаца там, где пустой строки быть не может",
  "elements.parbreak.template": "#parbreak()",

  "elements.list-bullet.name": "Маркированный список",
  "elements.list-bullet.summary": "Перечисление без порядка",
  "elements.list-bullet.template": "- ${Первый пункт}\n- ${Второй пункт}",

  "elements.list-numbered.name": "Нумерованный список",
  "elements.list-numbered.summary": "Перечисление по шагам",
  "elements.list-numbered.template": "+ ${Первый шаг}\n+ ${Второй шаг}",

  "elements.list-nested.name": "Вложенный список",
  "elements.list-nested.summary": "Уровень вложенности задаёт отступ",
  "elements.list-nested.template": "- ${Пункт}\n  - ${Вложенный пункт}",

  "elements.list-terms.name": "Список терминов",
  "elements.list-terms.summary": "Термин и его определение",
  "elements.list-terms.template": "/ ${Термин}: ${определение}",

  "elements.enum-numbering.name": "Нумерация списка",
  "elements.enum-numbering.summary": "Свой формат номеров: «a)», «I.», «1.1»",
  "elements.enum-numbering.template": "#set enum(numbering: \"${a)}\")",

  "elements.list-marker.name": "Маркер списка",
  "elements.list-marker.summary": "Чем помечены пункты вместо точки",
  "elements.list-marker.template": "#set list(marker: [${—}])",

  "elements.math-inline.name": "Формула в строке",
  "elements.math-inline.summary": "Формула внутри абзаца",
  "elements.math-inline.template": "$${x^2}$",

  "elements.math-block.name": "Формула блоком",
  "elements.math-block.summary": "Выключная формула отдельной строкой",
  "elements.math-block.template": "$ ${T(x) = T_0 + Delta T} $",

  "elements.math-numbering.name": "Нумерация формул",
  "elements.math-numbering.summary": "Номера у выключных формул — для ссылок @eq",
  "elements.math-numbering.template": "#set math.equation(numbering: \"(${1})\")",

  "elements.math-frac.name": "Дробь",
  "elements.math-frac.summary": "Числитель над знаменателем",
  "elements.math-frac.template": "frac(${a}, ${b})",

  "elements.math-sqrt.name": "Корень",
  "elements.math-sqrt.summary": "Квадратный корень",
  "elements.math-sqrt.template": "sqrt(${x})",

  "elements.math-sum.name": "Сумма",
  "elements.math-sum.summary": "Знак суммы с пределами",
  "elements.math-sum.template": "sum_(${i=1})^${n}",

  "elements.math-integral.name": "Интеграл",
  "elements.math-integral.summary": "Знак интеграла с пределами",
  "elements.math-integral.template": "integral_${a}^${b}",

  "elements.math-matrix.name": "Матрица",
  "elements.math-matrix.summary": "Строки через «;», элементы через «,»",
  "elements.math-matrix.template": "mat(${1}, ${2}; ${3}, ${4})",

  "elements.math-vector.name": "Вектор-столбец",
  "elements.math-vector.summary": "Столбец в круглых скобках",
  "elements.math-vector.template": "vec(${1}, ${2})",

  "elements.math-cases.name": "Система уравнений",
  "elements.math-cases.summary": "Фигурная скобка с вариантами",
  "elements.math-cases.template": "cases(${x = 1}, ${y = 2})",

  "elements.math-binom.name": "Биномиальный коэффициент",
  "elements.math-binom.summary": "«n по k» в круглых скобках",
  "elements.math-binom.template": "binom(${n}, ${k})",

  "elements.math-attach.name": "Индексы со всех сторон",
  "elements.math-attach.summary": "Верхние и нижние индексы слева и справа",
  "elements.math-attach.template": "attach(${A}, tl: ${1}, br: ${2})",

  "elements.math-accent.name": "Надстрочный знак",
  "elements.math-accent.summary": "Стрелка, крышка, тильда над символом",
  "elements.math-accent.template": "accent(${x}, ${arrow})",

  "elements.math-underbrace.name": "Фигурная скобка снизу",
  "elements.math-underbrace.summary": "Скобка с подписью под частью формулы",
  "elements.math-underbrace.template": "underbrace(${x + y}, \"${сумма}\")",

  "elements.math-cancel.name": "Сокращение",
  "elements.math-cancel.summary": "Перечёркивает сокращаемое",
  "elements.math-cancel.template": "cancel(${x})",

  "elements.math-lr.name": "Скобки по высоте",
  "elements.math-lr.summary": "Ограничители растут вместе с содержимым",
  "elements.math-lr.template": "lr(|${x}|)",

  "elements.math-op.name": "Имя оператора",
  "elements.math-op.summary": "Прямой шрифт для sin, cos, ker",
  "elements.math-op.template": "op(\"${sin}\")",

  "elements.math-bb.name": "Ажурный шрифт",
  "elements.math-bb.summary": "Множества: R, N, Z",
  "elements.math-bb.template": "bb(${R})",

  "elements.math-symbols.name": "Символы и структуры…",
  "elements.math-symbols.summary": "Палитра математических символов по имени",
  "elements.math-symbols.template": "",

  "elements.figure.name": "Рисунок",
  "elements.figure.summary": "Изображение с подписью и меткой для ссылок",
  "elements.figure.template": "#figure(\n  image(\"${data/график.png}\", width: ${80%}),\n  caption: [${Подпись}],\n) <fig:${label}>",

  "elements.figure-table.name": "Таблица с подписью",
  "elements.figure-table.summary": "Таблица в figure — нумеруется и ссылается",
  "elements.figure-table.template": "#figure(\n  table(columns: ${2}, [${A}], [${B}]),\n  caption: [${Подпись таблицы}],\n) <tbl:${label}>",

  "elements.table.name": "Таблица",
  "elements.table.summary": "Ячейки подряд, по столбцам",
  "elements.table.template": "#table(\n  columns: ${3},\n  [${A}], [${B}], [${C}],\n)",

  "elements.table-header.name": "Таблица с шапкой",
  "elements.table-header.summary": "Шапка повторяется на каждой странице",
  "elements.table-header.template": "#table(\n  columns: ${3},\n  table.header([${A}], [${B}], [${C}]),\n  [${1}], [${2}], [${3}],\n)",

  "elements.code-block.name": "Кодовый блок",
  "elements.code-block.summary": "Код с подсветкой по имени языка",
  "elements.code-block.template": "```${rust}\n${код}\n```",

  "elements.quote.name": "Цитата",
  "elements.quote.summary": "Выключная цитата с источником",
  "elements.quote.template": "#quote(block: true, attribution: [${Смирнов, 2023}])[\n  ${Текст цитаты.}\n]",

  "elements.block-fill.name": "Выделенный блок",
  "elements.block-fill.summary": "Абзац на подложке с отступами",
  "elements.block-fill.template": "#block(fill: luma(240), inset: 8pt)[\n  ${Важное замечание}\n]",

  "elements.page-setup.name": "Формат страницы",
  "elements.page-setup.summary": "Размер, поля, нумерация",
  "elements.page-setup.template": "#set page(paper: \"${a4}\", margin: ${2cm}, numbering: \"${1 / 1}\")",

  "elements.page-header.name": "Колонтитул",
  "elements.page-header.summary": "Строка вверху и внизу каждой страницы",
  "elements.page-header.template": "#set page(\n  header: align(${right})[${Шапка}],\n  footer: align(${center})[${Подвал}],\n)",

  "elements.columns.name": "Колонки",
  "elements.columns.summary": "Текст в несколько колонок",
  "elements.columns.template": "#columns(${2}, gutter: ${8pt})[\n  ${Текст в две колонки}\n]",

  "elements.colbreak.name": "Разрыв колонки",
  "elements.colbreak.summary": "Дальше — в следующей колонке",
  "elements.colbreak.template": "#colbreak()",

  "elements.grid.name": "Сетка",
  "elements.grid.summary": "Раскладка без табличных линеек",
  "elements.grid.template": "#grid(\n  columns: (${1fr}, ${1fr}),\n  gutter: ${8pt},\n  [${Слева}], [${Справа}],\n)",

  "elements.stack.name": "Стопка",
  "elements.stack.summary": "Элементы подряд в одном направлении",
  "elements.stack.template": "#stack(\n  dir: ${ttb},\n  spacing: ${8pt},\n  [${Первый}],\n  [${Второй}],\n)",

  "elements.align.name": "Выравнивание",
  "elements.align.summary": "center, right, horizon + left",
  "elements.align.template": "#align(${center})[${текст}]",

  "elements.pad.name": "Отступы",
  "elements.pad.summary": "Поля вокруг содержимого",
  "elements.pad.template": "#pad(x: ${2cm})[\n  ${Содержимое с отступами}\n]",

  "elements.place.name": "Свободное размещение",
  "elements.place.summary": "Вне потока: угол страницы, сдвиг, плавающий блок",
  "elements.place.template": "#place(${top + right}, dx: ${0pt}, dy: ${0pt})[${содержимое}]",

  "elements.box.name": "Бокс в строке",
  "elements.box.summary": "Кусок разметки внутри абзаца, не разрываемый",
  "elements.box.template": "#box(width: ${3cm})[${текст}]",

  "elements.hspace.name": "Горизонтальный отбив",
  "elements.hspace.summary": "Пробел заданной ширины; 1fr — распорка",
  "elements.hspace.template": "#h(${1fr})",

  "elements.vspace.name": "Вертикальный отбив",
  "elements.vspace.summary": "Пропуск по высоте между блоками",
  "elements.vspace.template": "#v(${1em})",

  "elements.repeat.name": "Заполнитель строки",
  "elements.repeat.summary": "Повторяет знак до конца строки — точки в оглавлении",
  "elements.repeat.template": "#repeat[${.}]",

  "elements.hide.name": "Невидимое содержимое",
  "elements.hide.summary": "Место занимает, на бумаге не видно",
  "elements.hide.template": "#hide[${текст}]",

  "elements.move.name": "Сдвиг",
  "elements.move.summary": "Смещает, не меняя раскладку вокруг",
  "elements.move.template": "#move(dx: ${4pt}, dy: ${0pt})[${текст}]",

  "elements.rotate.name": "Поворот",
  "elements.rotate.summary": "Поворачивает содержимое на угол",
  "elements.rotate.template": "#rotate(${45deg})[${текст}]",

  "elements.scale.name": "Масштаб",
  "elements.scale.summary": "Увеличивает или уменьшает содержимое",
  "elements.scale.template": "#scale(${150%})[${текст}]",

  "elements.image.name": "Изображение",
  "elements.image.summary": "Картинка из файла vault без подписи",
  "elements.image.template": "#image(\"${assets/схема.svg}\", width: ${80%})",

  "elements.rect.name": "Прямоугольник",
  "elements.rect.summary": "Рамка или плашка заданного размера",
  "elements.rect.template": "#rect(width: ${4cm}, height: ${2cm}, fill: ${luma(230)})[${текст}]",

  "elements.square.name": "Квадрат",
  "elements.square.summary": "Прямоугольник с равными сторонами",
  "elements.square.template": "#square(size: ${2cm}, fill: ${aqua})",

  "elements.circle.name": "Круг",
  "elements.circle.summary": "Круг по радиусу или по содержимому",
  "elements.circle.template": "#circle(radius: ${1cm}, fill: ${aqua})",

  "elements.ellipse.name": "Эллипс",
  "elements.ellipse.summary": "Овал заданной ширины и высоты",
  "elements.ellipse.template": "#ellipse(width: ${3cm}, height: ${2cm})",

  "elements.line.name": "Линия",
  "elements.line.summary": "Отрезок заданной длины и толщины",
  "elements.line.template": "#line(length: ${100%}, stroke: ${0.5pt + gray})",

  "elements.polygon.name": "Правильный многоугольник",
  "elements.polygon.summary": "Треугольник, пятиугольник, звезда основания",
  "elements.polygon.template": "#polygon.regular(size: ${2cm}, vertices: ${5}, fill: ${luma(230)})",

  "elements.gradient.name": "Градиентная заливка",
  "elements.gradient.summary": "Переход между цветами вместо сплошного фона",
  "elements.gradient.template": "#rect(width: ${100%}, height: ${1cm}, fill: gradient.linear(${blue}, ${red}))",

  "elements.set-text.name": "Шрифт документа",
  "elements.set-text.summary": "Гарнитура, кегль и язык переносов",
  "elements.set-text.template": "#set text(font: \"${New Computer Modern}\", size: ${11pt}, lang: \"${ru}\")",

  "elements.set-par.name": "Абзац",
  "elements.set-par.summary": "Выключка, интерлиньяж, красная строка",
  "elements.set-par.template": "#set par(justify: ${true}, leading: ${0.65em}, first-line-indent: ${1em})",

  "elements.set-heading.name": "Нумерация заголовков",
  "elements.set-heading.summary": "«1.1.» у разделов и подразделов",
  "elements.set-heading.template": "#set heading(numbering: \"${1.1.}\")",

  "elements.set-document.name": "Свойства документа",
  "elements.set-document.summary": "Название и автор — в PDF и в #title",
  "elements.set-document.template": "#set document(title: [${Название}], author: \"${Автор}\")",

  "elements.show-set.name": "Правило показа: стиль",
  "elements.show-set.summary": "Меняет оформление одного вида элементов",
  "elements.show-set.template": "#show heading.where(level: ${1}): set text(size: ${18pt})",

  "elements.show-function.name": "Правило показа: замена",
  "elements.show-function.summary": "Заворачивает элемент в свою разметку",
  "elements.show-function.template": "#show ${raw}: it => block(inset: ${6pt}, it)",

  "elements.show-regex.name": "Правило показа: по образцу",
  "elements.show-regex.summary": "Оформляет каждое совпадение в тексте",
  "elements.show-regex.template": "#show regex(\"${TODO}\"): strong",

  "elements.show-template.name": "Применить шаблон",
  "elements.show-template.summary": "Правило показа на весь документ целиком",
  "elements.show-template.template": "#show: ${шаблон}.with(title: [${Название}])",

  "elements.let-value.name": "Переменная",
  "elements.let-value.summary": "Значение под именем — набирать один раз",
  "elements.let-value.template": "#let ${имя} = ${значение}",

  "elements.let-function.name": "Своя функция",
  "elements.let-function.summary": "Разметка с параметрами под своим именем",
  "elements.let-function.template": "#let ${заметка}(тело) = block(fill: luma(240), inset: 8pt, тело)",

  "elements.import-package.name": "Пакет из Typst Universe",
  "elements.import-package.summary": "Внешняя библиотека по имени и версии",
  "elements.import-package.template": "#import \"@preview/${пакет}:${0.1.0}\": *",

  "elements.import-file.name": "Импорт файла",
  "elements.import-file.summary": "Определения из соседнего .typ",
  "elements.import-file.template": "#import \"${шаблон.typ}\": *",

  "elements.include-file.name": "Включить файл",
  "elements.include-file.summary": "Текст другого файла на это место",
  "elements.include-file.template": "#include \"${глава.typ}\"",

  "elements.context.name": "Контекстное выражение",
  "elements.context.summary": "Значение, известное только на месте вставки",
  "elements.context.template": "#context ${counter(page).display()}",

  "elements.data-csv.name": "Чтение CSV",
  "elements.data-csv.summary": "Таблица из файла как массив строк",
  "elements.data-csv.template": "#let ${данные} = csv(\"${data/таблица.csv}\")",

  "elements.data-json.name": "Чтение JSON",
  "elements.data-json.summary": "Словарь или массив из файла",
  "elements.data-json.template": "#let ${данные} = json(\"${data/файл.json}\")",

  "elements.data-yaml.name": "Чтение YAML",
  "elements.data-yaml.summary": "Данные из .yaml или .yml",
  "elements.data-yaml.template": "#let ${данные} = yaml(\"${data/файл.yaml}\")",

  "elements.data-read.name": "Чтение файла",
  "elements.data-read.summary": "Содержимое файла строкой",
  "elements.data-read.template": "#read(\"${заметки/фрагмент.txt}\")",

  "elements.data-table.name": "Таблица из CSV",
  "elements.data-table.summary": "Ячейки прямо из файла, без копирования",
  "elements.data-table.template": "#table(\n  columns: ${3},\n  ..csv(\"${data/таблица.csv}\").flatten(),\n)",

  "elements.counter-display.name": "Значение счётчика",
  "elements.counter-display.summary": "Номер страницы, раздела, рисунка на этом месте",
  "elements.counter-display.template": "#context counter(${page}).display()",

  "elements.counter-update.name": "Сброс счётчика",
  "elements.counter-update.summary": "Задаёт счётчику новое значение",
  "elements.counter-update.template": "#counter(${page}).update(${1})",

  "elements.numbering.name": "Формат номера",
  "elements.numbering.summary": "Число по образцу: «1.1», «I», «a»",
  "elements.numbering.template": "#numbering(\"${1.1}\", ${1}, ${2})",

  "elements.lorem.name": "Рыба текста",
  "elements.lorem.summary": "Заполнитель на заданное число слов",
  "elements.lorem.template": "#lorem(${50})",

  "elements.wiki-link.name": "Вики-ссылка",
  "elements.wiki-link.summary": "Ссылка на заметку vault по имени",
  "elements.wiki-link.template": "[[${Методология}]]",

  "elements.cross-ref.name": "Перекрёстная ссылка",
  "elements.cross-ref.summary": "Ссылка на метку внутри документа",
  "elements.cross-ref.template": "@${fig:thermogram}",

  "elements.label.name": "Метка",
  "elements.label.summary": "Имя места, на которое ссылаются",
  "elements.label.template": "<${sec:results}>",

  "elements.external-link.name": "Внешняя ссылка",
  "elements.external-link.summary": "Адрес в сети с видимым текстом",
  "elements.external-link.template": "#link(\"${https://typst.app}\")[${документация}]",

  "elements.cite.name": "Библиографическая ссылка",
  "elements.cite.summary": "Ссылка на запись из списка литературы",
  "elements.cite.template": "#cite(<${smirnov2023}>)",

  "elements.cite-supplement.name": "Ссылка с указанием места",
  "elements.cite-supplement.summary": "Страница или глава рядом с номером источника",
  "elements.cite-supplement.template": "#cite(<${smirnov2023}>, supplement: [${с. 42}])",

  "elements.bibliography.name": "Список литературы",
  "elements.bibliography.summary": "Записи из .bib или .yaml в выбранном стиле",
  "elements.bibliography.template": "#bibliography(\"${works.bib}\", style: \"${ieee}\")",

  "elements.footnote.name": "Сноска",
  "elements.footnote.summary": "Примечание внизу страницы",
  "elements.footnote.template": "#footnote[${Примечание}]",

  "elements.bold.name": "Жирный",
  "elements.bold.summary": "Полужирное начертание",
  "elements.bold.template": "*${текст}*",

  "elements.italic.name": "Курсив",
  "elements.italic.summary": "Наклонное начертание",
  "elements.italic.template": "_${текст}_",

  "elements.raw.name": "Моноширинный",
  "elements.raw.summary": "Код внутри строки",
  "elements.raw.template": "`${код}`",

  "elements.strike.name": "Зачёркнутый",
  "elements.strike.summary": "Линия поверх текста",
  "elements.strike.template": "#strike[${текст}]",

  "elements.underline.name": "Подчёркнутый",
  "elements.underline.summary": "Линия под текстом",
  "elements.underline.template": "#underline[${текст}]",

  "elements.overline.name": "Надчёркнутый",
  "elements.overline.summary": "Линия над текстом",
  "elements.overline.template": "#overline[${текст}]",

  "elements.highlight.name": "Маркер",
  "elements.highlight.summary": "Цветная подложка под словами",
  "elements.highlight.template": "#highlight[${текст}]",

  "elements.smallcaps.name": "Капитель",
  "elements.smallcaps.summary": "Строчные в виде малых прописных",
  "elements.smallcaps.template": "#smallcaps[${Текст}]",

  "elements.super.name": "Верхний индекс",
  "elements.super.summary": "Степень или знак сноски",
  "elements.super.template": "#super[${2}]",

  "elements.sub.name": "Нижний индекс",
  "elements.sub.summary": "Индекс под строкой",
  "elements.sub.template": "#sub[${2}]",

  "elements.upper.name": "Прописные",
  "elements.upper.summary": "Переводит текст в верхний регистр",
  "elements.upper.template": "#upper[${текст}]",

  "elements.text-style.name": "Цвет и насыщенность",
  "elements.text-style.summary": "Разовая правка шрифта на куске текста",
  "elements.text-style.template": "#text(fill: ${red}, weight: \"${bold}\")[${текст}]",

  "elements.linebreak.name": "Перенос строки",
  "elements.linebreak.summary": "Новая строка внутри абзаца",
  "elements.linebreak.template": "#linebreak()",

  "elements.comment.name": "Комментарий",
  "elements.comment.summary": "Заметка для себя, на бумагу не попадёт",
  "elements.comment.template": "// ${комментарий}",
};
