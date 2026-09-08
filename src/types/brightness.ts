export type BrightnessBackend = 'wmi' | 'ddc_high' | 'ddc_vcp';

export type ProbeState =
  | 'not_checked'
  | 'checking'
  | 'read_ok'
  | 'no_response'
  | 'unavailable'
  | 'ambiguous'
  | 'disconnected';

export type VerificationState =
  | 'not_tested'
  | 'testing'
  | 'device_confirmed'
  | 'user_confirmed'
  | 'failed'
  | 'recovery_pending'
  | 'stale';

export interface BrightnessError {
  code: string;
  native_code?: number | null;
  operation: string;
  retryable: boolean;
}

export interface BrightnessDisplay {
  id: string;
  generation: number;
  name: string;
  connection: string | null;
  is_primary: boolean;
  backend: BrightnessBackend | null;
  probe_state: ProbeState;
  verification: VerificationState;
  current_percent: number | null;
  target_percent: number | null;
  raw_min: number | null;
  raw_max: number | null;
  available_levels: number[] | null;
  busy: boolean;
  error: BrightnessError | null;
}

export interface BrightnessPreferences {
  sync_group: boolean;
  step_percent: number;
  group_display_ids: string[];
  hotkey_brightness_target: string;
  hotkey_brightness_up: string | null;
  hotkey_brightness_down: string | null;
  hotkey_brightness_screen: string | null;
}

export interface BrightnessStateSnapshot {
  revision: number;
  displays: BrightnessDisplay[];
  preferences: BrightnessPreferences;
  active_test_display_id: string | null;
}
