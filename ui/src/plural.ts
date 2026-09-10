const RU_PLURAL = new Intl.PluralRules("ru-RU");

/**
 * «1 заметка», «3 заметки», «9 заметок» — иначе подписи врут грамматикой.
 *
 * Через `Intl.PluralRules`, а не по остаткам от деления: правило выбора формы
 * принадлежит языку, и переписывать его руками в каждом компоненте — верный
 * способ получить три слегка разных правила.
 */
export function plural(count: number, one: string, few: string, many: string): string {
  switch (RU_PLURAL.select(count)) {
    case "one":
      return `${count} ${one}`;
    case "few":
      return `${count} ${few}`;
    default:
      return `${count} ${many}`;
  }
}
