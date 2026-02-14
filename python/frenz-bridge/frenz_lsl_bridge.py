"""
FRENZ LSL Bridge - Streams frenztoolkit data to Lab Streaming Layer.

This script is packaged as a standalone binary via PyApp and managed
by the HyperStudy Bridge Rust process manager.

Protocol:
  - Parent writes JSON config to stdin: {"device_id":"...","product_key":"..."}
  - This script prints JSON status lines to stdout
  - Parent sends "stop\n" to stdin for graceful shutdown
"""

import json
import signal
import sys
import tempfile
import threading
import time
import traceback

# Global shutdown event
shutdown_event = threading.Event()


def emit(status_dict):
    """Print a JSON status line to stdout for the parent process."""
    try:
        line = json.dumps(status_dict, separators=(",", ":"))
        sys.stdout.write(line + "\n")
        sys.stdout.flush()
    except Exception:
        pass


def stdin_listener():
    """Listen for 'stop' command on stdin."""
    try:
        for line in sys.stdin:
            if line.strip().lower() == "stop":
                shutdown_event.set()
                return
    except Exception:
        pass
    # stdin closed — treat as shutdown
    shutdown_event.set()


def create_outlets(device_id):
    """Create LSL outlets for all FRENZ stream types."""
    import pylsl

    outlets = {}

    # Stream definitions: (suffix, stream_type, channel_count, nominal_srate, channel_format)
    stream_defs = [
        # Raw physiological
        ("_EEG_raw", "EEG", 7, 125.0, pylsl.cf_float32),
        ("_PPG_raw", "PPG", 4, 25.0, pylsl.cf_float32),
        ("_IMU_raw", "IMU", 4, 50.0, pylsl.cf_float32),
        # Filtered physiological
        ("_EEG_filtered", "EEG", 7, 125.0, pylsl.cf_float32),
        ("_EOG_filtered", "EOG", 2, 125.0, pylsl.cf_float32),
        ("_EMG_filtered", "EMG", 2, 125.0, pylsl.cf_float32),
        # Metrics (scalar values, irregular rate)
        ("_focus", "Metrics", 1, 0.0, pylsl.cf_float32),
        ("_sleep_stage", "Metrics", 1, 0.0, pylsl.cf_float32),
        ("_poas", "Metrics", 1, 0.0, pylsl.cf_float32),
        ("_POSTURE", "Markers", 1, 0.0, pylsl.cf_string),
        ("_signal_quality", "Quality", 1, 0.0, pylsl.cf_float32),
        # Spectral power bands (5 channels: one per EEG electrode region)
        ("_alpha", "EEG", 5, 0.0, pylsl.cf_float32),
        ("_beta", "EEG", 5, 0.0, pylsl.cf_float32),
        ("_theta", "EEG", 5, 0.0, pylsl.cf_float32),
        ("_gamma", "EEG", 5, 0.0, pylsl.cf_float32),
        ("_delta", "EEG", 5, 0.0, pylsl.cf_float32),
    ]

    for suffix, stream_type, ch_count, srate, ch_format in stream_defs:
        stream_name = f"{device_id}{suffix}"
        info = pylsl.StreamInfo(
            name=stream_name,
            type=stream_type,
            channel_count=ch_count,
            nominal_srate=srate,
            channel_format=ch_format,
            source_id=f"frenz-bridge-{device_id}{suffix}",
        )
        outlets[suffix] = pylsl.StreamOutlet(info)

    return outlets


def push_raw_data(outlets, streamer):
    """Push raw physiological data from streamer.DATA to LSL outlets."""
    raw = streamer.DATA.get("RAW", {})

    # EEG raw: [N, 7] array
    eeg = raw.get("EEG")
    if eeg is not None and len(eeg) > 0:
        outlet = outlets.get("_EEG_raw")
        if outlet:
            for row in eeg:
                outlet.push_sample(row.tolist())

    # PPG raw: [N, 4] array
    ppg = raw.get("PPG")
    if ppg is not None and len(ppg) > 0:
        outlet = outlets.get("_PPG_raw")
        if outlet:
            for row in ppg:
                outlet.push_sample(row.tolist())

    # IMU raw: [N, 4] array
    imu = raw.get("IMU")
    if imu is not None and len(imu) > 0:
        outlet = outlets.get("_IMU_raw")
        if outlet:
            for row in imu:
                outlet.push_sample(row.tolist())


def push_filtered_data(outlets, streamer):
    """Push filtered physiological data from streamer.DATA to LSL outlets."""
    filtered = streamer.DATA.get("FILTERED", {})

    eeg = filtered.get("EEG")
    if eeg is not None and len(eeg) > 0:
        outlet = outlets.get("_EEG_filtered")
        if outlet:
            for row in eeg:
                outlet.push_sample(row.tolist())

    eog = filtered.get("EOG")
    if eog is not None and len(eog) > 0:
        outlet = outlets.get("_EOG_filtered")
        if outlet:
            for row in eog:
                outlet.push_sample(row.tolist())

    emg = filtered.get("EMG")
    if emg is not None and len(emg) > 0:
        outlet = outlets.get("_EMG_filtered")
        if outlet:
            for row in emg:
                outlet.push_sample(row.tolist())


def push_scores(outlets, streamer):
    """Push derived metrics and spectral data from streamer.SCORES to LSL outlets."""
    scores = streamer.SCORES
    if not scores:
        return

    # Scalar metrics
    scalar_map = {
        "_focus": "focus_score",
        "_sleep_stage": "sleep_stage",
        "_poas": "poas",
        "_signal_quality": "sqc_scores",
    }

    for suffix, key in scalar_map.items():
        val = scores.get(key)
        if val is not None:
            outlet = outlets.get(suffix)
            if outlet:
                try:
                    outlet.push_sample([float(val)])
                except (TypeError, ValueError):
                    pass

    # Posture (string stream)
    posture = scores.get("posture")
    if posture is not None:
        outlet = outlets.get("_POSTURE")
        if outlet:
            outlet.push_sample([str(posture)])

    # Spectral power bands (each is array of 5)
    band_map = {
        "_alpha": "alpha",
        "_beta": "beta",
        "_theta": "theta",
        "_gamma": "gamma",
        "_delta": "delta",
    }

    for suffix, key in band_map.items():
        val = scores.get(key)
        if val is not None:
            outlet = outlets.get(suffix)
            if outlet:
                try:
                    data = [float(v) for v in val]
                    if len(data) == 5:
                        outlet.push_sample(data)
                except (TypeError, ValueError):
                    pass


def main():
    """Main entry point for the FRENZ LSL bridge."""
    # Register signal handlers for graceful shutdown
    def handle_signal(_signum, _frame):
        shutdown_event.set()

    signal.signal(signal.SIGTERM, handle_signal)
    if hasattr(signal, "SIGINT"):
        signal.signal(signal.SIGINT, handle_signal)

    emit({"status": "waiting_for_config"})

    # Read credentials from stdin (single JSON line).
    # IMPORTANT: Read config BEFORE starting the stdin listener thread
    # to avoid a race condition where the thread consumes the config line.
    try:
        config_line = sys.stdin.readline()
        if not config_line.strip():
            emit({"status": "error", "message": "Empty config received on stdin"})
            sys.exit(1)
        config = json.loads(config_line)
        device_id = config["device_id"]
        product_key = config["product_key"]
    except (json.JSONDecodeError, KeyError) as e:
        emit({"status": "error", "message": f"Invalid config: {e}"})
        sys.exit(1)

    # Start stdin listener thread AFTER reading config to avoid race condition
    stdin_thread = threading.Thread(target=stdin_listener, daemon=True)
    stdin_thread.start()

    emit({"status": "bootstrapping", "phase": "importing", "package": "frenztoolkit"})

    try:
        from frenztoolkit import Streamer
    except ImportError as e:
        emit({"status": "error", "message": f"Failed to import frenztoolkit: {e}"})
        sys.exit(1)

    if shutdown_event.is_set():
        emit({"status": "stopped"})
        return

    emit({"status": "connecting", "device_id": device_id})

    # Create streamer with temp data folder
    data_folder = tempfile.mkdtemp(prefix="frenz_bridge_")
    try:
        streamer = Streamer(
            device_id=device_id,
            product_key=product_key,
            data_folder=data_folder,
            turn_off_light=True,
        )
    except Exception as e:
        emit({"status": "error", "message": f"Failed to create streamer: {e}"})
        sys.exit(1)

    if shutdown_event.is_set():
        emit({"status": "stopped"})
        return

    # Start the streamer (connects BLE + begins data flow)
    try:
        streamer.start()
    except Exception as e:
        emit({"status": "error", "message": f"Failed to start streamer: {e}"})
        sys.exit(1)

    # Wait for initial data to populate
    emit({"status": "connecting", "device_id": device_id, "phase": "waiting_for_data"})
    wait_start = time.time()
    while not shutdown_event.is_set() and time.time() - wait_start < 30:
        if streamer.DATA and streamer.DATA.get("RAW", {}).get("EEG") is not None:
            break
        time.sleep(0.5)

    if shutdown_event.is_set():
        try:
            streamer.stop()
        except Exception:
            pass
        emit({"status": "stopped"})
        return

    # Create LSL outlets
    emit({"status": "bootstrapping", "phase": "creating_outlets"})
    try:
        outlets = create_outlets(device_id)
    except Exception as e:
        emit({"status": "error", "message": f"Failed to create LSL outlets: {e}"})
        try:
            streamer.stop()
        except Exception:
            pass
        sys.exit(1)

    active_streams = list(outlets.keys())
    emit({"status": "streaming", "streams": active_streams, "sample_count": 0})

    # Main streaming loop
    sample_count = 0
    last_status_time = time.time()
    STATUS_INTERVAL = 5.0  # seconds between status updates

    try:
        while not shutdown_event.is_set():
            try:
                push_raw_data(outlets, streamer)
                push_filtered_data(outlets, streamer)
                push_scores(outlets, streamer)
                sample_count += 1
            except Exception as e:
                emit({"status": "error", "message": f"Push error: {e}"})

            # Periodic status update
            now = time.time()
            if now - last_status_time >= STATUS_INTERVAL:
                emit({
                    "status": "streaming",
                    "streams": active_streams,
                    "sample_count": sample_count,
                })
                last_status_time = now

            # Small sleep to avoid busy-waiting; actual pacing comes from
            # the frenztoolkit data arrival rate
            time.sleep(0.008)  # ~125 Hz max poll rate

    except Exception as e:
        emit({"status": "error", "message": f"Streaming error: {traceback.format_exc()}"})
    finally:
        # Clean shutdown
        try:
            streamer.stop()
        except Exception:
            pass
        emit({"status": "stopped"})


if __name__ == "__main__":
    main()
