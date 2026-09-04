export function reconcileTextEdit(
  original: string,
  normalizedOriginal: string,
  edited: string,
): string {
  if (!original.includes("\r")) return edited;
  if (normalizeLineEndings(original) === normalizedOriginal && edited === normalizedOriginal) {
    return original;
  }
  throw new Error("Text containing CR or CRLF line endings cannot be edited yet; its original bytes remain unchanged.");
}

export function normalizeLineEndings(value: string): string {
  return value.replace(/\r\n|\r/g, "\n");
}
