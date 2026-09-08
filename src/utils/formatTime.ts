export interface RelativeTimeDict {
  justNow: string;
  minAgo: string;
  hoursAgo: string;
}

export function formatRelativeTime(isoString: string, dict: RelativeTimeDict): string {
  const diff = Date.now() - new Date(isoString).getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return dict.justNow;
  if (minutes < 60) return `${minutes} ${dict.minAgo}`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} ${dict.hoursAgo}`;
  return new Date(isoString).toLocaleDateString();
}

export function formatClockTime(isoString: string): string {
  const d = new Date(isoString);
  const hours = d.getHours().toString().padStart(2, '0');
  const minutes = d.getMinutes().toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}
