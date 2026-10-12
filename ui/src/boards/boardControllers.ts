// Доски, смонтированные во вкладках, и итог их последней записи (фича 014,
// research.md §6, §8).
//
// Закрыть вкладку доски можно, только если она записана: ядро решает это по
// `Close.board_unsaved`, но состояние сохранения доски держит машина
// `boards::save`, значение которой живёт в компоненте вкладки. Этот реестр —
// мост между вкладкой и тем, кто закрывает: он умеет дописать доску и сказать,
// удалось ли. Своих правил здесь нет — признак «не записана» уходит в ядро как
// есть.

export interface BoardController {
  /** Записать доску немедленно. `true` — на диске актуальная версия. */
  flush(): Promise<boolean>;
  /** Показать на полотне другую версию — принятую при разрешении конфликта. */
  replace(next: string): void;
}

export class BoardControllers {
  private readonly mounted = new Map<string, BoardController>();
  /** Доски, чья запись при уходе со вкладки не удалась: правки остались только в памяти окна. */
  private readonly failed = new Set<string>();

  /** Вкладка доски смонтирована. Возвращает снятие. */
  register(path: string, controller: BoardController): () => void {
    this.mounted.set(path, controller);
    // Свежее чтение с диска — новая правда о доске.
    this.failed.delete(path);
    return () => {
      if (this.mounted.get(path) === controller) this.mounted.delete(path);
    };
  }

  /** Вкладка доски ушла, а запись не удалась. */
  noteFailure(path: string): void {
    this.failed.add(path);
  }

  /** Есть ли доска, которую нельзя оставить: незаписанное в памяти окна. */
  hasFailed(): boolean {
    return this.failed.size > 0;
  }

  /**
   * Дописать доску по пути. Нет вкладки — решает итог прошлой записи: доска, чей
   * уход закончился отказом, остаётся «не записанной».
   */
  async flush(path: string): Promise<boolean> {
    const controller = this.mounted.get(path);
    if (controller === undefined) return !this.failed.has(path);
    const ok = await controller.flush();
    if (ok) this.failed.delete(path);
    else this.failed.add(path);
    return ok;
  }

  /** Принятая при конфликте версия доски — на полотно, если вкладка смонтирована. */
  replace(path: string, next: string): void {
    this.mounted.get(path)?.replace(next);
  }

  /** Вкладка закрыта: помнить о ней нечего. */
  forget(path: string): void {
    this.failed.delete(path);
  }
}
