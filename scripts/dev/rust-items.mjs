// Разбор текста Rust на элементы верхнего уровня — для скриптов переноса и
// генерации обёрток команд (фича 037).

/** Делит текст Rust на элементы верхнего уровня (с их комментариями и атрибутами). */
export function scanItems(text) {
  const items = [];
  let depth = 0;
  let start = 0;
  let i = 0;
  const n = text.length;
  const close = (end) => {
    items.push(text.slice(start, end));
    start = end;
  };
  while (i < n) {
    const c = text[i];
    const next = text[i + 1];
    if (c === "/" && next === "/") {
      const eol = text.indexOf("\n", i);
      i = eol === -1 ? n : eol + 1;
      continue;
    }
    if (c === "/" && next === "*") {
      let level = 1;
      i += 2;
      while (i < n && level > 0) {
        if (text[i] === "/" && text[i + 1] === "*") {
          level++;
          i += 2;
        } else if (text[i] === "*" && text[i + 1] === "/") {
          level--;
          i += 2;
        } else i++;
      }
      continue;
    }
    if (c === "r" && (next === '"' || (next === "#" && /r#+"/.test(text.slice(i, i + 10)))) && !/\w/.test(text[i - 1] ?? "")) {
      const m = /^r(#*)"/.exec(text.slice(i));
      const end = text.indexOf('"' + m[1], i + m[0].length);
      i = end + 1 + m[1].length;
      continue;
    }
    if (c === '"') {
      i++;
      while (i < n && text[i] !== '"') i += text[i] === "\\" ? 2 : 1;
      i++;
      continue;
    }
    if (c === "'") {
      if (next === "\\") {
        i = text.indexOf("'", i + 3) + 1;
        continue;
      }
      if (text[i + 2] === "'") {
        i += 3;
        continue;
      }
      i++;
      continue;
    }
    if (c === "{" || c === "(" || c === "[") depth++;
    else if (c === "}" || c === ")" || c === "]") {
      depth--;
      if (depth === 0 && c === "}") {
        let j = i + 1;
        while (j < n && /[ \t]/.test(text[j])) j++;
        if (text[j] === ";") i = j;
        const eol = text.indexOf("\n", i);
        close(eol === -1 ? n : eol + 1);
        i = eol === -1 ? n : eol + 1;
        continue;
      }
    } else if (c === ";" && depth === 0) {
      const eol = text.indexOf("\n", i);
      close(eol === -1 ? n : eol + 1);
      i = eol === -1 ? n : eol + 1;
      continue;
    }
    i++;
  }
  if (start < n) items.push(text.slice(start));
  return items;
}

/** Текст элемента без ведущих комментариев и атрибутов. */
export function body(item) {
  return item
    .replace(/^(\s*(\/\/[^\n]*\n|#\[[^\n]*\]\s*\n|#\[[\s\S]*?\]\n))*/, "")
    .trim();
}

export function head(item) {
  const m =
    /^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(fn|struct|enum|type|const|static|mod|macro_rules!|impl|use|trait)\s*(?:<[^>]*>\s*)?([A-Za-z_]\w*)?/.exec(
      body(item),
    );
  return m ? { kind: m[1], name: m[2] ?? "" } : { kind: "?", name: "" };
}
