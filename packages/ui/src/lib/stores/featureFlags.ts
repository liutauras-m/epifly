export interface FeatureFlags {
  recorder: boolean;
  tabs: boolean;
  traceReplay: boolean;
}

const defaults: FeatureFlags = {
  recorder: false,
  tabs: false,
  traceReplay: false,
};

export function createFeatureFlags(overrides: Partial<FeatureFlags> = {}) {
  let flags = $state<FeatureFlags>({ ...defaults, ...overrides });

  return {
    get recorder() { return flags.recorder; },
    get tabs() { return flags.tabs; },
    get traceReplay() { return flags.traceReplay; },
    enable(flag: keyof FeatureFlags) { flags[flag] = true; },
    disable(flag: keyof FeatureFlags) { flags[flag] = false; },
  };
}

export type FeatureFlagStore = ReturnType<typeof createFeatureFlags>;
