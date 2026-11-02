//! Инструкции учителя и описания инструментов (фича 051, research.md §2, §7).
//! Встроенный текст на двух языках; язык общения — поле шапки урока. Правка
//! инструкций пользователем — фича 059.

use serde_json::{json, Value};

use super::file::Language;
use crate::ai::settings::AiProvider;

pub const ASK_QUESTION: &str = "ask_question";
pub const READ_NOTE: &str = "read_note";

const SYSTEM_RU: &str = "\
Ты — терпеливый учитель. Цель урока задал пользователь; твоя задача — довести его до \
понимания, а не пересказать тему. Общайся по-русски.

Метод каждого шага: предсказание, объяснение, проверка.
1. Предсказание. Прежде чем что-то объяснять, попроси пользователя предположить: что \
получится, как это устроено, почему. Даже неверное предположение показывает, что он \
уже знает и где ошибается.
2. Объяснение. Объясняй коротко, на том, что пользователь уже проверенно понял, и \
связывай новое с его собственными словами. Один шаг — одна мысль. Не вываливай всю тему \
сразу и не читай лекцию.
3. Проверка. Закончи шаг вопросом, ответ на который покажет, понял ли он на самом деле, \
а не запомнил формулировку: применить на новом примере, предсказать результат, найти \
ошибку.

Правила:
- Не двигайся к следующему шагу, пока не разобрана путаница в текущем. Если ответ \
неверный или неуверенный, найди, какое представление за ним стоит, и разбери именно его, \
не повторяя прежнего объяснения теми же словами.
- Учитывай уверенность, с которой пользователь ответил. Уверенный неверный ответ — \
устойчивое заблуждение, разбирай его первым. Верный ответ «наугад» не считай \
пониманием: попроси объяснить, почему.
- Если пользователь приложил «рассуждение вслух», опирайся на него: отвечай на ход мысли, \
а не только на итог.
- Каждый вопрос к пользователю задавай инструментом ask_question: вопрос и, когда \
уместно, варианты ответа (2–5). Свободный вопрос задавай без вариантов. Один вопрос за \
раз, и после него жди ответа.
- Заметки пользователя читай инструментом read_note только когда без них не обойтись \
или пользователь сам сослался на заметку. Не выдумывай содержимое заметок, которых не \
читал. Если доступ к заметке закрыт или её нет, скажи об этом и продолжай без неё.
- Хвали за конкретное, а не вообще. Не торопи и не стыди за ошибки.
- Когда цель достигнута и проверена, скажи об этом и коротко подведи итог.";

const SYSTEM_EN: &str = "\
You are a patient teacher. The user set the goal of the lesson; your job is to bring \
them to real understanding, not to recite the topic. Talk in English.

The method of every step: prediction, explanation, check.
1. Prediction. Before explaining anything, ask the user to predict: what will happen, \
how it works, why. Even a wrong guess shows what they already know and where they err.
2. Explanation. Explain briefly, building on what the user has already shown they \
understand, and tie the new idea to their own words. One step, one idea. Do not dump the \
whole topic or lecture.
3. Check. End the step with a question whose answer shows whether they truly understood \
rather than memorised a phrase: apply it to a new example, predict a result, find a \
mistake.

Rules:
- Do not move to the next step until the confusion in the current one is resolved. If the \
answer is wrong or unsure, find the mental model behind it and address exactly that, \
without repeating the earlier explanation in the same words.
- Take the user's stated confidence into account. A confident wrong answer is a stable \
misconception: handle it first. A correct answer given as a guess is not understanding: \
ask them to explain why.
- If the user attached \"thinking aloud\", build on it: answer the line of thought, not \
only the result.
- Ask every question to the user with the ask_question tool: the question and, when it \
fits, answer choices (2-5). Ask an open question without choices. One question at a time, \
then wait for the answer.
- Read the user's notes with the read_note tool only when you cannot do without them or \
the user refers to a note. Never invent the contents of notes you did not read. If access \
to a note is closed or it does not exist, say so and go on without it.
- Praise what is specific, not in general. Do not rush or shame mistakes.
- When the goal is reached and checked, say so and give a short summary.";

const COMPACT_RU: &str = "\
Изложи пройденное в уроке, чтобы урок можно было продолжить по твоему изложению без \
прежней переписки. Включи: цель урока; что пользователь проверенно понял; где он \
путался и что из этого осталось неразобранным; какие заметки обсуждались; на каком шаге \
остановились и что идёт дальше. Только факты урока, без вопросов и обращений, до \
400 слов.";

const COMPACT_EN: &str = "\
Summarise what has been covered so far so the lesson can continue from your summary \
without the earlier conversation. Include: the goal of the lesson; what the user has \
shown they understand; where they were confused and what is still unresolved; which \
notes were discussed; which step we stopped at and what comes next. Facts of the lesson \
only, no questions and no addressing the user, up to 400 words.";

pub fn system(language: Language, goal: &str) -> String {
    match language {
        Language::Ru => format!("{SYSTEM_RU}\n\nЦель урока: {goal}"),
        Language::En => format!("{SYSTEM_EN}\n\nGoal of the lesson: {goal}"),
    }
}

pub fn compact_request(language: Language) -> &'static str {
    match language {
        Language::Ru => COMPACT_RU,
        Language::En => COMPACT_EN,
    }
}

/// Первое сообщение урока, когда пользователь ещё ничего не говорил.
pub fn start_message(language: Language) -> &'static str {
    match language {
        Language::Ru => "Начни урок.",
        Language::En => "Start the lesson.",
    }
}

pub fn continue_message(language: Language) -> &'static str {
    match language {
        Language::Ru => "Продолжай.",
        Language::En => "Continue.",
    }
}

/// Заголовок блока с прикреплённой заметкой в инструкциях.
pub fn attached_heading(language: Language) -> &'static str {
    match language {
        Language::Ru => "Заметки, которые пользователь прикрепил к уроку",
        Language::En => "Notes the user attached to the lesson",
    }
}

pub fn summary_heading(language: Language) -> &'static str {
    match language {
        Language::Ru => "Изложение пройденного ранее",
        Language::En => "Summary of what was covered earlier",
    }
}

pub fn question_label(language: Language) -> &'static str {
    match language {
        Language::Ru => "Вопрос",
        Language::En => "Question",
    }
}

pub fn confidence_label(language: Language, confidence: super::file::Confidence) -> &'static str {
    use super::file::Confidence::{Guess, Likely, Sure};
    match (language, confidence) {
        (Language::Ru, Guess) => "Уверенность: угадываю",
        (Language::Ru, Likely) => "Уверенность: скорее всего так",
        (Language::Ru, Sure) => "Уверенность: уверен",
        (Language::En, Guess) => "Confidence: guessing",
        (Language::En, Likely) => "Confidence: probably",
        (Language::En, Sure) => "Confidence: sure",
    }
}

pub fn thinking_label(language: Language) -> &'static str {
    match language {
        Language::Ru => "Рассуждение вслух",
        Language::En => "Thinking aloud",
    }
}

/// Результат `read_note`, который уходит модели следующим ходом.
pub fn note_result(language: Language, path: &str, outcome: NoteResult<'_>) -> String {
    match (language, outcome) {
        (Language::Ru, NoteResult::Text { text, truncated }) => format!(
            "Результат read_note({path}):\n{text}{}",
            if truncated {
                "\n[заметка обрезана]"
            } else {
                ""
            }
        ),
        (Language::En, NoteResult::Text { text, truncated }) => format!(
            "Result of read_note({path}):\n{text}{}",
            if truncated { "\n[note truncated]" } else { "" }
        ),
        (Language::Ru, NoteResult::Denied) => {
            format!("Результат read_note({path}): доступ к заметке закрыт пользователем.")
        }
        (Language::En, NoteResult::Denied) => {
            format!("Result of read_note({path}): access to the note is closed by the user.")
        }
        (Language::Ru, NoteResult::Missing) => {
            format!("Результат read_note({path}): такой заметки нет.")
        }
        (Language::En, NoteResult::Missing) => {
            format!("Result of read_note({path}): there is no such note.")
        }
    }
}

pub enum NoteResult<'a> {
    Text { text: &'a str, truncated: bool },
    Denied,
    Missing,
}

/// Описания инструментов в форме протокола подключения.
pub fn tools(provider: AiProvider) -> Value {
    let ask = (
        ASK_QUESTION,
        "Ask the user a question and wait for the answer. Use it for every question to the user. \
         Provide `choices` for a multiple-choice question; omit it for an open question.",
        json!({
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The question text."},
                "choices": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Answer choices, 2-5; omit for an open question."
                }
            },
            "required": ["question"]
        }),
    );
    let read = (
        READ_NOTE,
        "Read one of the user's notes by its path relative to the vault. Access may be refused.",
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Note path relative to the vault."}
            },
            "required": ["path"]
        }),
    );
    let list = [ask, read];
    match provider {
        AiProvider::Anthropic => Value::Array(
            list.into_iter()
                .map(|(name, description, schema)| {
                    json!({"name": name, "description": description, "input_schema": schema})
                })
                .collect(),
        ),
        AiProvider::OpenAi | AiProvider::Custom => Value::Array(
            list.into_iter()
                .map(|(name, description, schema)| {
                    json!({"type": "function", "function": {
                        "name": name, "description": description, "parameters": schema}})
                })
                .collect(),
        ),
    }
}
