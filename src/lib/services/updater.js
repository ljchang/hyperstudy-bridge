import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

// Holds the plugin's Update object privately so callers never touch it
let pendingUpdate = null;

/**
 * Check if an update is available.
 * @returns {{ available: boolean, version?: string, body?: string, error?: string }}
 */
export async function checkForUpdate() {
  try {
    const update = await check();
    if (update) {
      console.log(`Update available: ${update.currentVersion} -> ${update.version}`);
      pendingUpdate = update;
      return {
        available: true,
        version: update.version,
        body: update.body,
        date: update.date,
        currentVersion: update.currentVersion,
      };
    }
    pendingUpdate = null;
    return { available: false };
  } catch (error) {
    console.error('Update check failed:', error);
    pendingUpdate = null;
    return { available: false, error: error.message || String(error) };
  }
}

/**
 * Download and install the pending update with progress tracking.
 * @param {(progress: { phase: string, downloaded?: number, contentLength?: number, percent?: number }) => void} onProgress
 */
export async function downloadAndInstallUpdate(onProgress) {
  if (!pendingUpdate) throw new Error('No pending update');

  let downloaded = 0;
  let contentLength = 0;

  await pendingUpdate.downloadAndInstall(event => {
    switch (event.event) {
      case 'Started':
        contentLength = event.data.contentLength || 0;
        onProgress?.({ phase: 'started', contentLength });
        break;
      case 'Progress':
        downloaded += event.data.chunkLength;
        onProgress?.({
          phase: 'downloading',
          downloaded,
          contentLength,
          percent: contentLength > 0 ? Math.round((downloaded / contentLength) * 100) : 0,
        });
        break;
      case 'Finished':
        onProgress?.({ phase: 'finished' });
        break;
    }
  });

  pendingUpdate = null;
}

export async function relaunchApp() {
  await relaunch();
}
