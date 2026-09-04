export async function copyText(value: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
    return;
  } catch {
    const fallback = document.createElement("textarea");
    fallback.value = value;
    fallback.setAttribute("readonly", "");
    fallback.style.position = "fixed";
    fallback.style.opacity = "0";
    fallback.style.pointerEvents = "none";
    const previousFocus = document.activeElement;
    // Nodes outside a modal dialog are inert and cannot be selected for fallback copying.
    (document.querySelector("dialog[open]") ?? document.body).appendChild(fallback);
    try {
      fallback.select();
      if (!document.execCommand("copy")) throw new Error("clipboard permission denied");
    } finally {
      fallback.remove();
      if (previousFocus instanceof HTMLElement) previousFocus.focus({ preventScroll: true });
    }
  }
}
