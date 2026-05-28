// See https://svelte.dev/docs/kit/types#app.d.ts
declare global {
  namespace App {
    // interface Error {}
    interface Locals {
      userId?: string;
      session?: {
        userIss: string;
        userSub: string;
        tenantOrgId: string;
        displayName: string;
        emailVerified: boolean;
        accessToken: string;
      };
    }
    // interface PageData {}
    // interface PageState {}
    // interface Platform {}
  }
}

export {};
