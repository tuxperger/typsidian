// Телефонная раскладка (фича 010, T057, SC-006): ширина 400 px, vault с заметкой.
// Снимки — `SHOT_A` (до открытия), `SHOT_B` (текст); в журнал — ширина страницы
// и элементы, вышедшие за экран (внутри прокручиваемой шапки это допустимо).
//
//   SHOT_A=/tmp/a.png SHOT_B=/tmp/b.png nix develop -c node scripts/dev/browser.mjs \
//       http://localhost:3000 scripts/dev/scenarios/phone-layout.mjs --opfs-vault

const write = (name, text) => `(async () => {
  const root = await navigator.storage.getDirectory();
  const vault = await root.getDirectoryHandle("vault", { create: true });
  const file = await vault.getFileHandle(${JSON.stringify(name)}, { create: true });
  const stream = await file.createWritable();
  await stream.write(${JSON.stringify(text)});
  await stream.close();
})()`;
export default async (page) => {
  await page.viewport(400, 820);
  await page.evaluate(write("Заметка.typ", "= Заголовок\nТекст заметки для проверки телефонной раскладки.\n"));
  await page.evaluate(write("Вторая.typ", "= Вторая\n"));
  await page.click("text=Открыть vault");
  await page.sleep(1500);
  await page.shot(process.env.SHOT_A);
  await page.click("text=Заметка");
  await page.sleep(2500);
  await page.shot(process.env.SHOT_B);
  const overflow = await page.evaluate(`JSON.stringify({w: document.documentElement.scrollWidth, vw: innerWidth, wide: [...document.querySelectorAll("body *")].filter(e => { const r = e.getBoundingClientRect(); return r.width > 0 && (r.right > innerWidth + 1); }).slice(0, 12).map(e => (e.className && e.className.baseVal === undefined ? e.className : e.tagName) + " " + Math.round(e.getBoundingClientRect().right))})`);
  page.log(overflow);
  if (JSON.parse(overflow).w > 400) throw new Error("страница шире экрана");
};
