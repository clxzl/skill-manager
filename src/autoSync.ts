export interface AutoSyncSetting {
  enabled: boolean;
  minutes: number;
}

export const AUTO_SYNC_INTERVALS = [5, 10, 30, 60];

const KEY = "autoSync";
const DEFAULT_MINUTES = 10;

export function getAutoSync(): AutoSyncSetting {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<AutoSyncSetting> | null;
    const minutes = value?.minutes;
    return {
      enabled: value?.enabled === true,
      minutes:
        typeof minutes === "number" && AUTO_SYNC_INTERVALS.includes(minutes)
          ? minutes
          : DEFAULT_MINUTES,
    };
  } catch {
    return { enabled: false, minutes: DEFAULT_MINUTES };
  }
}

export function setAutoSync(setting: AutoSyncSetting) {
  localStorage.setItem(KEY, JSON.stringify(setting));
}