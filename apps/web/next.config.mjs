/**
 * Оболочка web-клиента (plan.md → Project Structure). Вся логика живёт в
 * `packages/core` и приходит сюда через `@typsidian/ui` — здесь только то, без
 * чего Next.js не соберёт эти два источника вместе.
 *
 * @type {import('next').NextConfig}
 */
const nextConfig = {
  // `@typsidian/ui` собирается своим `tsc` в `ui/dist` и приходит сюда уже
  // обычным ESM — поэтому ни `transpilePackages`, ни разрешение расширений
  // `.js -> .ts` здесь не нужны. Внутри `ui/` импорты записаны с `.js`, как
  // того требует ESM, и сборщику остаётся резолвить их буквально.

  // Turbopack (в Next 16 — сборщик по умолчанию) умеет и `.wasm`, и
  // `new URL(..., import.meta.url)`, которыми пользуется сборка wasm-pack под
  // target=web, поэтому отдельной настройки загрузчика не требуется. Пустой
  // объект здесь не декоративен: он фиксирует выбор сборщика явно.
  turbopack: {},
};

export default nextConfig;
