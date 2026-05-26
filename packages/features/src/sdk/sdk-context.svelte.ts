import { getContext, setContext } from "svelte";
import type { ConusSdk } from "@conusai/sdk";

const SDK_CONTEXT = Symbol("conus-sdk");

export function setSdkContext(sdk: ConusSdk): void {
  setContext(SDK_CONTEXT, sdk);
}

export function getSdkContext(): ConusSdk {
  const sdk = getContext<ConusSdk | undefined>(SDK_CONTEXT);
  if (!sdk) throw new Error("Conus SDK context is missing. Wrap your app with SdkProvider.");
  return sdk;
}
