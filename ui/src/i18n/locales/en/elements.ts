import type { Entries } from "../define.js";

/**
 * Каталог элементов Typst (меню и панель вставки): название, описание и шаблон.
 * Шаблон — код Typst; переводятся только заглушки полей `${…}` — их текст
 * попадает в заметку, и он на языке интерфейса.
 */
export const elements = {
  "elements.category.struct": "Structure",
  "elements.category.list": "Lists",
  "elements.category.math": "Math",
  "elements.category.block": "Blocks",
  "elements.category.layout": "Layout",
  "elements.category.draw": "Graphics",
  "elements.category.style": "Styles",
  "elements.category.data": "Data",
  "elements.category.link": "Links",
  "elements.category.mark": "Markup",

  "elements.heading-1.name": "Heading 1",
  "elements.heading-1.summary": "Top-level section",
  "elements.heading-1.template": "= ${Heading}",

  "elements.heading-2.name": "Heading 2",
  "elements.heading-2.summary": "Subsection",
  "elements.heading-2.template": "== ${Subheading}",

  "elements.heading-3.name": "Heading 3",
  "elements.heading-3.summary": "Item within a subsection",
  "elements.heading-3.template": "=== ${Item}",

  "elements.heading-4.name": "Heading 4",
  "elements.heading-4.summary": "Sub-item",
  "elements.heading-4.template": "==== ${Sub-item}",

  "elements.title.name": "Document title",
  "elements.title.summary": "The document’s name, once per document",
  "elements.title.template": "#set document(title: [${Title}])\n#title()",

  "elements.outline.name": "Table of contents",
  "elements.outline.summary": "Contents by headings down to the given depth",
  "elements.outline.template": "#outline(title: [${Contents}], depth: ${2})",

  "elements.divider.name": "Thematic break",
  "elements.divider.summary": "A rule between parts of the text",
  "elements.divider.template": "#divider()",

  "elements.pagebreak.name": "Page break",
  "elements.pagebreak.summary": "Continue on the next page",
  "elements.pagebreak.template": "#pagebreak()",

  "elements.parbreak.name": "Paragraph break",
  "elements.parbreak.summary": "End a paragraph where a blank line can’t go",
  "elements.parbreak.template": "#parbreak()",

  "elements.list-bullet.name": "Bulleted list",
  "elements.list-bullet.summary": "Items in no particular order",
  "elements.list-bullet.template": "- ${First item}\n- ${Second item}",

  "elements.list-numbered.name": "Numbered list",
  "elements.list-numbered.summary": "Step-by-step items",
  "elements.list-numbered.template": "+ ${First step}\n+ ${Second step}",

  "elements.list-nested.name": "Nested list",
  "elements.list-nested.summary": "Indentation sets the nesting level",
  "elements.list-nested.template": "- ${Item}\n  - ${Nested item}",

  "elements.list-terms.name": "Term list",
  "elements.list-terms.summary": "A term and its definition",
  "elements.list-terms.template": "/ ${Term}: ${definition}",

  "elements.enum-numbering.name": "List numbering",
  "elements.enum-numbering.summary": "Custom number format: “a)”, “I.”, “1.1”",
  "elements.enum-numbering.template": "#set enum(numbering: \"${a)}\")",

  "elements.list-marker.name": "List marker",
  "elements.list-marker.summary": "What marks items instead of a bullet",
  "elements.list-marker.template": "#set list(marker: [${—}])",

  "elements.math-inline.name": "Inline math",
  "elements.math-inline.summary": "A formula inside a paragraph",
  "elements.math-inline.template": "$${x^2}$",

  "elements.math-block.name": "Display math",
  "elements.math-block.summary": "A formula on its own line",
  "elements.math-block.template": "$ ${T(x) = T_0 + Delta T} $",

  "elements.math-numbering.name": "Equation numbering",
  "elements.math-numbering.summary": "Numbers on display equations — for @eq references",
  "elements.math-numbering.template": "#set math.equation(numbering: \"(${1})\")",

  "elements.math-frac.name": "Fraction",
  "elements.math-frac.summary": "Numerator over denominator",
  "elements.math-frac.template": "frac(${a}, ${b})",

  "elements.math-sqrt.name": "Root",
  "elements.math-sqrt.summary": "Square root",
  "elements.math-sqrt.template": "sqrt(${x})",

  "elements.math-sum.name": "Sum",
  "elements.math-sum.summary": "Summation sign with limits",
  "elements.math-sum.template": "sum_(${i=1})^${n}",

  "elements.math-integral.name": "Integral",
  "elements.math-integral.summary": "Integral sign with limits",
  "elements.math-integral.template": "integral_${a}^${b}",

  "elements.math-matrix.name": "Matrix",
  "elements.math-matrix.summary": "Rows separated by “;”, entries by “,”",
  "elements.math-matrix.template": "mat(${1}, ${2}; ${3}, ${4})",

  "elements.math-vector.name": "Column vector",
  "elements.math-vector.summary": "A column in parentheses",
  "elements.math-vector.template": "vec(${1}, ${2})",

  "elements.math-cases.name": "Cases",
  "elements.math-cases.summary": "A curly brace with alternatives",
  "elements.math-cases.template": "cases(${x = 1}, ${y = 2})",

  "elements.math-binom.name": "Binomial coefficient",
  "elements.math-binom.summary": "“n choose k” in parentheses",
  "elements.math-binom.template": "binom(${n}, ${k})",

  "elements.math-attach.name": "Attachments on all sides",
  "elements.math-attach.summary": "Super- and subscripts on the left and right",
  "elements.math-attach.template": "attach(${A}, tl: ${1}, br: ${2})",

  "elements.math-accent.name": "Accent",
  "elements.math-accent.summary": "Arrow, hat, tilde over a symbol",
  "elements.math-accent.template": "accent(${x}, ${arrow})",

  "elements.math-underbrace.name": "Underbrace",
  "elements.math-underbrace.summary": "A brace with a label under part of a formula",
  "elements.math-underbrace.template": "underbrace(${x + y}, \"${sum}\")",

  "elements.math-cancel.name": "Cancel",
  "elements.math-cancel.summary": "Strikes out what cancels",
  "elements.math-cancel.template": "cancel(${x})",

  "elements.math-lr.name": "Scaled delimiters",
  "elements.math-lr.summary": "Delimiters grow with their content",
  "elements.math-lr.template": "lr(|${x}|)",

  "elements.math-op.name": "Operator name",
  "elements.math-op.summary": "Upright font for sin, cos, ker",
  "elements.math-op.template": "op(\"${sin}\")",

  "elements.math-bb.name": "Blackboard bold",
  "elements.math-bb.summary": "Sets: R, N, Z",
  "elements.math-bb.template": "bb(${R})",

  "elements.math-symbols.name": "Symbols and structures…",
  "elements.math-symbols.summary": "Math symbol palette by name",
  "elements.math-symbols.template": "",

  "elements.figure.name": "Figure",
  "elements.figure.summary": "An image with a caption and a label for references",
  "elements.figure.template": "#figure(\n  image(\"${data/chart.png}\", width: ${80%}),\n  caption: [${Caption}],\n) <fig:${label}>",

  "elements.figure-table.name": "Table with caption",
  "elements.figure-table.summary": "A table in a figure — numbered and referenceable",
  "elements.figure-table.template": "#figure(\n  table(columns: ${2}, [${A}], [${B}]),\n  caption: [${Table caption}],\n) <tbl:${label}>",

  "elements.table.name": "Table",
  "elements.table.summary": "Cells in sequence, by columns",
  "elements.table.template": "#table(\n  columns: ${3},\n  [${A}], [${B}], [${C}],\n)",

  "elements.table-header.name": "Table with header",
  "elements.table-header.summary": "The header repeats on every page",
  "elements.table-header.template": "#table(\n  columns: ${3},\n  table.header([${A}], [${B}], [${C}]),\n  [${1}], [${2}], [${3}],\n)",

  "elements.code-block.name": "Code block",
  "elements.code-block.summary": "Code highlighted by language name",
  "elements.code-block.template": "```${rust}\n${code}\n```",

  "elements.quote.name": "Quote",
  "elements.quote.summary": "A block quote with attribution",
  "elements.quote.template": "#quote(block: true, attribution: [${Smith, 2023}])[\n  ${Quoted text.}\n]",

  "elements.block-fill.name": "Highlighted block",
  "elements.block-fill.summary": "A paragraph on a shaded background with padding",
  "elements.block-fill.template": "#block(fill: luma(240), inset: 8pt)[\n  ${Important note}\n]",

  "elements.page-setup.name": "Page format",
  "elements.page-setup.summary": "Size, margins, numbering",
  "elements.page-setup.template": "#set page(paper: \"${a4}\", margin: ${2cm}, numbering: \"${1 / 1}\")",

  "elements.page-header.name": "Header and footer",
  "elements.page-header.summary": "A line at the top and bottom of every page",
  "elements.page-header.template": "#set page(\n  header: align(${right})[${Header}],\n  footer: align(${center})[${Footer}],\n)",

  "elements.columns.name": "Columns",
  "elements.columns.summary": "Text in several columns",
  "elements.columns.template": "#columns(${2}, gutter: ${8pt})[\n  ${Text in two columns}\n]",

  "elements.colbreak.name": "Column break",
  "elements.colbreak.summary": "Continue in the next column",
  "elements.colbreak.template": "#colbreak()",

  "elements.grid.name": "Grid",
  "elements.grid.summary": "Layout without table rules",
  "elements.grid.template": "#grid(\n  columns: (${1fr}, ${1fr}),\n  gutter: ${8pt},\n  [${Left}], [${Right}],\n)",

  "elements.stack.name": "Stack",
  "elements.stack.summary": "Items in a row in one direction",
  "elements.stack.template": "#stack(\n  dir: ${ttb},\n  spacing: ${8pt},\n  [${First}],\n  [${Second}],\n)",

  "elements.align.name": "Alignment",
  "elements.align.summary": "center, right, horizon + left",
  "elements.align.template": "#align(${center})[${text}]",

  "elements.pad.name": "Padding",
  "elements.pad.summary": "Space around the content",
  "elements.pad.template": "#pad(x: ${2cm})[\n  ${Padded content}\n]",

  "elements.place.name": "Free placement",
  "elements.place.summary": "Out of flow: page corner, offset, floating block",
  "elements.place.template": "#place(${top + right}, dx: ${0pt}, dy: ${0pt})[${content}]",

  "elements.box.name": "Inline box",
  "elements.box.summary": "An unbreakable piece of markup inside a paragraph",
  "elements.box.template": "#box(width: ${3cm})[${text}]",

  "elements.hspace.name": "Horizontal spacing",
  "elements.hspace.summary": "A gap of given width; 1fr is a spring",
  "elements.hspace.template": "#h(${1fr})",

  "elements.vspace.name": "Vertical spacing",
  "elements.vspace.summary": "A vertical gap between blocks",
  "elements.vspace.template": "#v(${1em})",

  "elements.repeat.name": "Line filler",
  "elements.repeat.summary": "Repeats a mark to the end of the line — dots in a table of contents",
  "elements.repeat.template": "#repeat[${.}]",

  "elements.hide.name": "Hidden content",
  "elements.hide.summary": "Takes up space but isn’t visible on paper",
  "elements.hide.template": "#hide[${text}]",

  "elements.move.name": "Move",
  "elements.move.summary": "Shifts content without changing the layout around it",
  "elements.move.template": "#move(dx: ${4pt}, dy: ${0pt})[${text}]",

  "elements.rotate.name": "Rotate",
  "elements.rotate.summary": "Rotates content by an angle",
  "elements.rotate.template": "#rotate(${45deg})[${text}]",

  "elements.scale.name": "Scale",
  "elements.scale.summary": "Enlarges or shrinks content",
  "elements.scale.template": "#scale(${150%})[${text}]",

  "elements.image.name": "Image",
  "elements.image.summary": "A picture from a vault file, without a caption",
  "elements.image.template": "#image(\"${assets/diagram.svg}\", width: ${80%})",

  "elements.rect.name": "Rectangle",
  "elements.rect.summary": "A frame or panel of given size",
  "elements.rect.template": "#rect(width: ${4cm}, height: ${2cm}, fill: ${luma(230)})[${text}]",

  "elements.square.name": "Square",
  "elements.square.summary": "A rectangle with equal sides",
  "elements.square.template": "#square(size: ${2cm}, fill: ${aqua})",

  "elements.circle.name": "Circle",
  "elements.circle.summary": "A circle by radius or by content",
  "elements.circle.template": "#circle(radius: ${1cm}, fill: ${aqua})",

  "elements.ellipse.name": "Ellipse",
  "elements.ellipse.summary": "An oval of given width and height",
  "elements.ellipse.template": "#ellipse(width: ${3cm}, height: ${2cm})",

  "elements.line.name": "Line",
  "elements.line.summary": "A segment of given length and thickness",
  "elements.line.template": "#line(length: ${100%}, stroke: ${0.5pt + gray})",

  "elements.polygon.name": "Regular polygon",
  "elements.polygon.summary": "Triangle, pentagon, star base",
  "elements.polygon.template": "#polygon.regular(size: ${2cm}, vertices: ${5}, fill: ${luma(230)})",

  "elements.gradient.name": "Gradient fill",
  "elements.gradient.summary": "A blend between colors instead of a solid fill",
  "elements.gradient.template": "#rect(width: ${100%}, height: ${1cm}, fill: gradient.linear(${blue}, ${red}))",

  "elements.set-text.name": "Document font",
  "elements.set-text.summary": "Typeface, size and hyphenation language",
  "elements.set-text.template": "#set text(font: \"${New Computer Modern}\", size: ${11pt}, lang: \"${en}\")",

  "elements.set-par.name": "Paragraph",
  "elements.set-par.summary": "Justification, leading, first-line indent",
  "elements.set-par.template": "#set par(justify: ${true}, leading: ${0.65em}, first-line-indent: ${1em})",

  "elements.set-heading.name": "Heading numbering",
  "elements.set-heading.summary": "“1.1.” on sections and subsections",
  "elements.set-heading.template": "#set heading(numbering: \"${1.1.}\")",

  "elements.set-document.name": "Document properties",
  "elements.set-document.summary": "Title and author — in the PDF and in #title",
  "elements.set-document.template": "#set document(title: [${Title}], author: \"${Author}\")",

  "elements.show-set.name": "Show rule: style",
  "elements.show-set.summary": "Changes the look of one kind of element",
  "elements.show-set.template": "#show heading.where(level: ${1}): set text(size: ${18pt})",

  "elements.show-function.name": "Show rule: replace",
  "elements.show-function.summary": "Wraps an element in your own markup",
  "elements.show-function.template": "#show ${raw}: it => block(inset: ${6pt}, it)",

  "elements.show-regex.name": "Show rule: by pattern",
  "elements.show-regex.summary": "Styles every match in the text",
  "elements.show-regex.template": "#show regex(\"${TODO}\"): strong",

  "elements.show-template.name": "Apply template",
  "elements.show-template.summary": "A show rule for the whole document",
  "elements.show-template.template": "#show: ${template}.with(title: [${Title}])",

  "elements.let-value.name": "Variable",
  "elements.let-value.summary": "A value under a name — type it once",
  "elements.let-value.template": "#let ${name} = ${value}",

  "elements.let-function.name": "Custom function",
  "elements.let-function.summary": "Markup with parameters under your own name",
  "elements.let-function.template": "#let ${note}(body) = block(fill: luma(240), inset: 8pt, body)",

  "elements.import-package.name": "Package from Typst Universe",
  "elements.import-package.summary": "An external library by name and version",
  "elements.import-package.template": "#import \"@preview/${package}:${0.1.0}\": *",

  "elements.import-file.name": "Import file",
  "elements.import-file.summary": "Definitions from a neighbouring .typ",
  "elements.import-file.template": "#import \"${template.typ}\": *",

  "elements.include-file.name": "Include file",
  "elements.include-file.summary": "Another file’s text in this place",
  "elements.include-file.template": "#include \"${chapter.typ}\"",

  "elements.context.name": "Context expression",
  "elements.context.summary": "A value known only where it’s inserted",
  "elements.context.template": "#context ${counter(page).display()}",

  "elements.data-csv.name": "Read CSV",
  "elements.data-csv.summary": "A table from a file as an array of rows",
  "elements.data-csv.template": "#let ${data} = csv(\"${data/table.csv}\")",

  "elements.data-json.name": "Read JSON",
  "elements.data-json.summary": "A dictionary or array from a file",
  "elements.data-json.template": "#let ${data} = json(\"${data/file.json}\")",

  "elements.data-yaml.name": "Read YAML",
  "elements.data-yaml.summary": "Data from .yaml or .yml",
  "elements.data-yaml.template": "#let ${data} = yaml(\"${data/file.yaml}\")",

  "elements.data-read.name": "Read file",
  "elements.data-read.summary": "A file’s contents as a string",
  "elements.data-read.template": "#read(\"${notes/fragment.txt}\")",

  "elements.data-table.name": "Table from CSV",
  "elements.data-table.summary": "Cells straight from a file, no copying",
  "elements.data-table.template": "#table(\n  columns: ${3},\n  ..csv(\"${data/table.csv}\").flatten(),\n)",

  "elements.counter-display.name": "Counter value",
  "elements.counter-display.summary": "Page, section or figure number at this place",
  "elements.counter-display.template": "#context counter(${page}).display()",

  "elements.counter-update.name": "Reset counter",
  "elements.counter-update.summary": "Gives a counter a new value",
  "elements.counter-update.template": "#counter(${page}).update(${1})",

  "elements.numbering.name": "Number format",
  "elements.numbering.summary": "A number by pattern: “1.1”, “I”, “a”",
  "elements.numbering.template": "#numbering(\"${1.1}\", ${1}, ${2})",

  "elements.lorem.name": "Placeholder text",
  "elements.lorem.summary": "Filler of a given number of words",
  "elements.lorem.template": "#lorem(${50})",

  "elements.wiki-link.name": "Wiki link",
  "elements.wiki-link.summary": "A link to a vault note by name",
  "elements.wiki-link.template": "[[${Methodology}]]",

  "elements.cross-ref.name": "Cross-reference",
  "elements.cross-ref.summary": "A reference to a label within the document",
  "elements.cross-ref.template": "@${fig:thermogram}",

  "elements.label.name": "Label",
  "elements.label.summary": "A name for a place that is referenced",
  "elements.label.template": "<${sec:results}>",

  "elements.external-link.name": "External link",
  "elements.external-link.summary": "A web address with visible text",
  "elements.external-link.template": "#link(\"${https://typst.app}\")[${documentation}]",

  "elements.cite.name": "Citation",
  "elements.cite.summary": "A reference to a bibliography entry",
  "elements.cite.template": "#cite(<${smirnov2023}>)",

  "elements.cite-supplement.name": "Citation with location",
  "elements.cite-supplement.summary": "A page or chapter next to the source number",
  "elements.cite-supplement.template": "#cite(<${smirnov2023}>, supplement: [${p. 42}])",

  "elements.bibliography.name": "Bibliography",
  "elements.bibliography.summary": "Entries from .bib or .yaml in the chosen style",
  "elements.bibliography.template": "#bibliography(\"${works.bib}\", style: \"${ieee}\")",

  "elements.footnote.name": "Footnote",
  "elements.footnote.summary": "A note at the bottom of the page",
  "elements.footnote.template": "#footnote[${Note}]",

  "elements.bold.name": "Bold",
  "elements.bold.summary": "Bold weight",
  "elements.bold.template": "*${text}*",

  "elements.italic.name": "Italic",
  "elements.italic.summary": "Slanted style",
  "elements.italic.template": "_${text}_",

  "elements.raw.name": "Monospace",
  "elements.raw.summary": "Inline code",
  "elements.raw.template": "`${code}`",

  "elements.strike.name": "Strikethrough",
  "elements.strike.summary": "A line through the text",
  "elements.strike.template": "#strike[${text}]",

  "elements.underline.name": "Underline",
  "elements.underline.summary": "A line under the text",
  "elements.underline.template": "#underline[${text}]",

  "elements.overline.name": "Overline",
  "elements.overline.summary": "A line over the text",
  "elements.overline.template": "#overline[${text}]",

  "elements.highlight.name": "Highlight",
  "elements.highlight.summary": "A colored background behind words",
  "elements.highlight.template": "#highlight[${text}]",

  "elements.smallcaps.name": "Small caps",
  "elements.smallcaps.summary": "Lowercase as small capitals",
  "elements.smallcaps.template": "#smallcaps[${Text}]",

  "elements.super.name": "Superscript",
  "elements.super.summary": "A power or footnote mark",
  "elements.super.template": "#super[${2}]",

  "elements.sub.name": "Subscript",
  "elements.sub.summary": "An index below the line",
  "elements.sub.template": "#sub[${2}]",

  "elements.upper.name": "Uppercase",
  "elements.upper.summary": "Converts text to upper case",
  "elements.upper.template": "#upper[${text}]",

  "elements.text-style.name": "Color and weight",
  "elements.text-style.summary": "A one-off font change on a piece of text",
  "elements.text-style.template": "#text(fill: ${red}, weight: \"${bold}\")[${text}]",

  "elements.linebreak.name": "Line break",
  "elements.linebreak.summary": "A new line inside a paragraph",
  "elements.linebreak.template": "#linebreak()",

  "elements.comment.name": "Comment",
  "elements.comment.summary": "A note to yourself, won’t reach the paper",
  "elements.comment.template": "// ${comment}",
} as const satisfies Entries;
