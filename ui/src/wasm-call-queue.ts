// Очередь вызовов к одному объекту ядра за границей wasm-bindgen.
//
// Экспортированный `async`-метод с `&mut self` держит заимствование
// Rust-объекта всё время, пока его future не завершится, — а завершается она
// через несколько await'ов на промисах браузера. Любой второй вызов того же
// объекта в это окно — включая синхронный — падает в рантайме:
//
//     recursive use of an object detected which would lead to unsafe aliasing in rust
//
// Окно не гипотетическое: цикл сверки с диском тикает раз в секунду и обходит
// весь vault, дёргая `getFile()` на каждый файл. Он пересекается практически с
// любым действием пользователя — созданием заметки, автосохранением,
// применением настройки.
//
// Поэтому доступ к одному объекту ядра сериализован: строго один вызов за раз,
// в порядке поступления. Это не бизнес-логика (принцип I) — это правило
// владения объектом на конкретной платформенной границе, и живёт оно там же,
// где сама граница. Нативному клиенту очередь не нужна: там ядро за границей
// процесса, и каждая команда Tauri берёт свой мьютекс.

export interface WasmCallQueue {
  /** Вызов ядра по запросу пользователя. */
  run<T>(op: () => T | Promise<T>): Promise<T>;
  /**
   * Вызов из собственного цикла опроса. От `run` отличается ровно одним: не
   * учитывается в `hasPendingUserCalls`, потому что на вопрос «есть ли у
   * пользователя что терять» такой вызов не влияет.
   */
  runInternal<T>(op: () => T | Promise<T>): Promise<T>;
  /**
   * Занято ли ядро. Пока `true`, синхронно обращаться к объекту нельзя: вызов
   * упадёт на заимствовании.
   */
  isBusy(): boolean;
  /** Есть ли незавершённый вызов, начатый пользователем. */
  hasPendingUserCalls(): boolean;
}

export function createWasmCallQueue(): WasmCallQueue {
  let tail: Promise<void> = Promise.resolve();
  let pending = 0;
  let pendingUser = 0;

  function enqueue<T>(op: () => T | Promise<T>, byUser: boolean): Promise<T> {
    pending += 1;
    if (byUser) pendingUser += 1;

    const started = tail.then(op);
    const release = (): void => {
      pending -= 1;
      if (byUser) pendingUser -= 1;
    };
    // Отказ одной операции не рвёт очередь: следующая ждёт завершения
    // предыдущей независимо от исхода. Сама ошибка при этом не теряется — она
    // уходит вызывающей стороне через `started`.
    tail = started.then(release, release);
    return started;
  }

  return {
    run<T>(op: () => T | Promise<T>): Promise<T> {
      return enqueue(op, true);
    },
    runInternal<T>(op: () => T | Promise<T>): Promise<T> {
      return enqueue(op, false);
    },
    isBusy: () => pending > 0,
    hasPendingUserCalls: () => pendingUser > 0,
  };
}
