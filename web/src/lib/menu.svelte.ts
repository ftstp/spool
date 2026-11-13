// The app's right-click menu: one at a time, opened where the pointer is. A page builds the
// items; the menu draws them (sub-menus one level deep), Escape or a click elsewhere closes.
export type MenuItem =
  | { label: string; run?: () => void; danger?: boolean; disabled?: boolean; hint?: string; sub?: MenuItem[]; checked?: boolean }
  | { sep: true };

export const menu = $state<{ open: boolean; x: number; y: number; items: MenuItem[]; title: string }>({
  open: false,
  x: 0,
  y: 0,
  items: [],
  title: "",
});

export function openMenu(e: MouseEvent, items: MenuItem[], title = "") {
  e.preventDefault();
  e.stopPropagation();
  menu.items = items;
  menu.title = title;
  menu.x = e.clientX;
  menu.y = e.clientY;
  menu.open = true;
}

export function closeMenu() {
  menu.open = false;
}
