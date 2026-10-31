const ICONS = {
  stack:
    '<path d="M12 3.5l8 4.3-8 4.2-8-4.2 8-4.3z"/><path d="M4 12.2l8 4.2 8-4.2"/><path d="M4 16.4l8 4.1 8-4.1"/>',
  branch:
    '<circle cx="6" cy="18" r="2.1"/><circle cx="6" cy="6" r="2.1"/><circle cx="18" cy="18" r="2.1"/><path d="M6 8.1v7.8"/><path d="M8.1 6H12a6 6 0 016 6v3.9"/>',
  sync:
    '<path d="M20 11a8 8 0 00-14-4.5L4 9"/><path d="M4 4.2V9h4.8"/><path d="M4 13a8 8 0 0014 4.5l2-2.5"/><path d="M20 19.8V15h-4.8"/>',
  gear:
    '<circle cx="12" cy="12" r="3"/><path d="M19.2 12c0-.4 0-.7-.1-1.1l2-1.5-2-3.4-2.4 1a7.7 7.7 0 00-1.9-1.1L14.4 3h-4l-.4 2.9a7.7 7.7 0 00-1.9 1.1l-2.4-1-2 3.4 2 1.5a7.7 7.7 0 000 2.2l-2 1.5 2 3.4 2.4-1a7.7 7.7 0 001.9 1.1l.4 2.9h4l.4-2.9a7.7 7.7 0 001.9-1.1l2.4 1 2-3.4-2-1.5c.1-.4.1-.7.1-1.1z"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.4-3.4"/>',
  chev: '<path d="M9 18l6-6-6-6"/>',
  back: '<path d="M15 18l-6-6 6-6"/>',
  link:
    '<path d="M10 13a4.2 4.2 0 006 .3l1.9-1.9a4.2 4.2 0 00-6-6l-1 .9"/><path d="M14 11a4.2 4.2 0 00-6-.3l-1.9 1.9a4.2 4.2 0 006 6l1-.9"/>',
  copy:
    '<rect x="8.5" y="8.5" width="11" height="11" rx="2"/><path d="M5 15.5H4.5A1.5 1.5 0 013 14V5.5A1.5 1.5 0 014.5 4H13a1.5 1.5 0 011.5 1.5V6"/>',
  dash: '<path d="M6 12h12"/>',
  warn:
    '<path d="M12 9v4.2"/><path d="M12 17h.01"/><path d="M10.3 4.2L2.5 17.7A2 2 0 004.2 20.7h15.6a2 2 0 001.7-3L13.7 4.2a2 2 0 00-3.4 0z"/>',
  folder:
    '<path d="M3.5 7.2a2 2 0 012-2h4l2 2h7a2 2 0 012 2v7.6a2 2 0 01-2 2h-13a2 2 0 01-2-2V7.2z"/>',
  cloud:
    '<path d="M17.3 18.5a4.3 4.3 0 00.9-8.5 6.2 6.2 0 00-11.9-1 4.3 4.3 0 00.7 9.5h10.3z"/>',
  clock:
    '<circle cx="12" cy="12" r="8.6"/><path d="M12 7.3v5l3.1 1.9"/>',
  lock:
    '<rect x="4.5" y="10" width="15" height="10" rx="2"/><path d="M8 10V7.3a4 4 0 018 0V10"/>',
  open:
    '<path d="M14 4h6v6"/><path d="M20 4L10.5 13.5"/><path d="M19.5 14.5V19a1.5 1.5 0 01-1.5 1.5H5A1.5 1.5 0 013.5 19V5.5A1.5 1.5 0 015 4h4.5"/>',
  plus: '<path d="M12 5v14"/><path d="M5 12h14"/>',
  archive:
    '<rect x="3.5" y="4.5" width="17" height="4" rx="1"/><path d="M5 8.5V18a1.5 1.5 0 001.5 1.5h11A1.5 1.5 0 0019 18V8.5"/><path d="M10 12h4"/>',
} as const;

export type IconName = keyof typeof ICONS;

export function Icon({
  name,
  className,
}: {
  name: IconName;
  className?: string;
}) {
  return (
    <svg
      className={`i ${className ?? ""}`}
      viewBox="0 0 24 24"
      aria-hidden="true"
      dangerouslySetInnerHTML={{ __html: ICONS[name] }}
    />
  );
}
