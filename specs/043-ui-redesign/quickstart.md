# Quickstart: Оболочка интерфейса по дизайну Typsidian.pdf

```bash
nix develop -c pnpm dev
nix develop -c node scripts/dev/browser.mjs http://localhost:3000 \
    scripts/dev/scenarios/redesign.mjs --opfs-vault --timeout 90000
```

Сценарий снимает экраны 1–4 (рабочая область 1200×750, стартовый экран, телефон 360×740:
дерево и документ), проверяет состав шапки, переключение вида, меню vault, низ дерева,
полосу предпросмотра и недавние vault. Снимки сверяются с Typsidian.pdf.
