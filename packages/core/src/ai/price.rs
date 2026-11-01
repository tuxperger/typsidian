//! Справочник цен (FR-014): только проверенные цены, по скиллу `claude-api`,
//! кэш от 2026-10-06, в долларах за миллион токенов. Нет в таблице — стоимость
//! неизвестна, а не ноль. Цен OpenAI из проверенного источника нет.

const PRICES: &[(&str, f64, f64)] = &[
    ("claude-fable-5-1", 10.0, 50.0),
    ("claude-fable-5", 10.0, 50.0),
    ("claude-opus-5-5", 4.0, 20.0),
    ("claude-opus-5", 5.0, 25.0),
    ("claude-opus-4-8", 5.0, 25.0),
    ("claude-opus-4-7", 5.0, 25.0),
    ("claude-opus-4-6", 5.0, 25.0),
    ("claude-sonnet-5-5", 2.0, 10.0),
    ("claude-sonnet-5", 2.0, 10.0),
    ("claude-sonnet-4-6", 3.0, 15.0),
    // Базовый тариф, до 100K токенов на запрос.
    ("claude-haiku-5-5", 0.10, 0.50),
    ("claude-haiku-4-5", 1.0, 5.0),
];

pub fn estimate_usd(model: &str, input_tokens: u64, output_tokens: u64) -> Option<f64> {
    let (_, input, output) = PRICES.iter().find(|(id, _, _)| *id == model)?;
    Some((input_tokens as f64 * input + output_tokens as f64 * output) / 1_000_000.0)
}
