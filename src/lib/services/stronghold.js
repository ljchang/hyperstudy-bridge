import { Stronghold } from '@tauri-apps/plugin-stronghold';
import { appDataDir } from '@tauri-apps/api/path';

// Lazy-initialized singleton
let storePromise = null;
let strongholdInstance = null;

const VAULT_PASSWORD = 'hyperstudy-bridge-vault';
const CLIENT_NAME = 'hyperstudy-bridge';

async function getStore() {
  if (!storePromise) {
    storePromise = initStore();
  }
  return storePromise;
}

async function initStore() {
  const dataDir = await appDataDir();
  const vaultPath = `${dataDir}/vault.hold`;

  strongholdInstance = await Stronghold.load(vaultPath, VAULT_PASSWORD);

  let client;
  try {
    client = await strongholdInstance.loadClient(CLIENT_NAME);
  } catch {
    client = await strongholdInstance.createClient(CLIENT_NAME);
  }

  return client.getStore();
}

/**
 * Store a secret string in the encrypted vault.
 * @param {string} key - The key to store under
 * @param {string} value - The secret value to store
 */
export async function setSecret(key, value) {
  const store = await getStore();
  const data = Array.from(new TextEncoder().encode(value));
  await store.insert(key, data);
  await strongholdInstance.save();
}

/**
 * Retrieve a secret string from the encrypted vault.
 * @param {string} key - The key to retrieve
 * @returns {Promise<string|null>} The secret value, or null if not found
 */
export async function getSecret(key) {
  const store = await getStore();
  try {
    const data = await store.get(key);
    if (!data || data.length === 0) return null;
    return new TextDecoder().decode(new Uint8Array(data));
  } catch {
    return null;
  }
}

/**
 * Remove a secret from the encrypted vault.
 * @param {string} key - The key to remove
 */
export async function removeSecret(key) {
  const store = await getStore();
  try {
    await store.remove(key);
    await strongholdInstance.save();
  } catch {
    // Key didn't exist — that's fine
  }
}
