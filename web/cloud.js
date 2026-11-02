// The cloud side: one client, one row per user, one JSON document.
//
// The save lives in Postgres, not in the browser. Local storage is kept only as
// a read-through cache so a cold start with no network still opens the timer
// instead of a blank page — it is never the source of truth, and every write
// goes to the server.

const PUBLIC_CONFIG = {
  endpoint: 'https://pomodoro-36310.app.workbuddy.host',
  oauthRelayBaseUrl: 'https://www.workbuddy.cn/v2/as/genie-baas/oauth',
  publishableKey: 'wbpk_PAb06CARldeQ5eqHS2W9Jl_vvAkYChJ7uYLkCrGm3i5RyKmmXL59Wtl',
};

const TABLE = 'pomodoro_saves';
const CACHE_KEY = 'bongocat.pomodoro.web.v1';

let client = null;

function sdk() {
  const global = window.WorkBuddyCloud;
  if (!global?.createWorkBuddyCloud) {
    throw new Error('云端 SDK 未载入，检查网络后刷新');
  }
  return global;
}

export function cloud() {
  if (!client) {
    client = sdk().createWorkBuddyCloud({
      endpoint: PUBLIC_CONFIG.endpoint,
      oauthRelayBaseUrl: PUBLIC_CONFIG.oauthRelayBaseUrl,
      publishableKey: PUBLIC_CONFIG.publishableKey,
    });
  }
  return client;
}

/* ---------- auth ---------- */

export async function session() {
  const { data, error } = await cloud().auth.getSession();
  if (error) return null;
  return data ?? null;
}

export async function sendCode(email) {
  const { data, error } = await cloud().auth.sendOtp({ email });
  if (error) throw error;
  return data;
}

export async function verifyCode({ email, verificationId, isExistingUser, token }) {
  const { data, error } = await cloud().auth.verifyOtp({
    email,
    verificationId,
    isExistingUser,
    token,
  });
  if (error) throw error;
  return data;
}

export async function signOut() {
  await cloud().auth.signOut();
}

/* ---------- the save ---------- */

// The document as stored, or null when this user has never saved.
export async function loadSave() {
  const { data, error } = await cloud()
    .database.from(TABLE)
    .select('document, saved_at')
    .maybeSingle();
  if (error) throw error;
  if (!data) return null;
  return { document: JSON.stringify(data.document), savedAt: data.saved_at };
}

// Upsert this user's row. `owner_id` is never sent: the column defaults to
// auth.uid() and the RLS policy rejects anything else.
export async function saveSave(documentText) {
  const payload = { document: JSON.parse(documentText), saved_at: new Date().toISOString() };
  const { error } = await cloud()
    .database.from(TABLE)
    .upsert(payload, { onConflict: 'owner_id' });
  if (error) throw error;
}

/* ---------- the cache ---------- */

// Only ever a fallback for reading. Writes go to `saveSave`.
export const cache = {
  read() {
    try {
      return window.localStorage.getItem(CACHE_KEY) || '';
    } catch {
      return '';
    }
  },
  write(text) {
    try {
      window.localStorage.setItem(CACHE_KEY, text);
    } catch {
      // A blocked store costs the offline copy, not the save.
    }
  },
  clear() {
    try {
      window.localStorage.removeItem(CACHE_KEY);
    } catch {
      // Nothing to do.
    }
  },
};

export const emailOf = (current) => current?.user?.email ?? current?.user?.name ?? '已登录';
