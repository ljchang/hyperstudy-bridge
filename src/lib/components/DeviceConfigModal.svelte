<script>

  // Use Svelte 5 $props() rune
  let { isOpen = false, device = null, onSave = () => {}, onClose = () => {} } = $props();

  // Form state using $state rune
  let formData = $state({});
  let lslConfig = $state({});
  let errors = $state({});
  let isSubmitting = $state(false);
  let activeTab = $state('device'); // 'device' or 'lsl'

  // Device configuration templates with validation rules
  const deviceConfigs = {
    ttl: {
      port: {
        label: 'Serial Port',
        type: 'text',
        placeholder: '/dev/ttyUSB0 (Linux) or /dev/tty.usbmodem (macOS)',
        required: true,
        pattern: '^(/dev/tty\\.|/dev/ttyUSB|COM\\d+)',
        errorMessage: 'Invalid port format. Expected /dev/ttyUSB0, /dev/tty.usbmodem1234, or COM1'
      },
      baudRate: {
        label: 'Baud Rate',
        type: 'select',
        options: [9600, 19200, 38400, 57600, 115200],
        default: 115200,
        required: true
      },
      pulseDuration: {
        label: 'Pulse Duration (ms)',
        type: 'number',
        min: 1,
        max: 1000,
        default: 10,
        required: true,
        errorMessage: 'Duration must be between 1-1000ms'
      }
    },
    kernel: {
      ip: {
        label: 'IP Address',
        type: 'text',
        placeholder: '127.0.0.1 or 192.168.1.100',
        required: true,
        pattern: '^((25[0-5]|(2[0-4]|1\\d|[1-9]|)\\d)\\.?\\b){4}$',
        errorMessage: 'Invalid IP address format'
      },
      port: {
        label: 'Port',
        type: 'number',
        min: 1,
        max: 65535,
        default: 6767,
        required: true,
        errorMessage: 'Port must be between 1-65535'
      },
      samplingRate: {
        label: 'Sampling Rate (Hz)',
        type: 'select',
        options: [1, 5, 10, 25, 50, 100],
        default: 10,
        required: true
      }
    },
    pupil: {
      url: {
        label: 'Device URL',
        type: 'text',
        placeholder: 'localhost:8081 or 192.168.1.101:8081',
        required: true,
        pattern: '^[\\w.-]+(:\\d+)?$',
        errorMessage: 'Invalid URL format. Expected hostname:port'
      },
      streamGaze: {
        label: 'Stream Gaze Data',
        type: 'checkbox',
        default: true
      },
      streamVideo: {
        label: 'Stream Video',
        type: 'checkbox',
        default: false
      },
      gazeFormat: {
        label: 'Gaze Data Format',
        type: 'select',
        options: ['normalized', 'pixels', 'degrees'],
        default: 'normalized',
        required: true
      }
    },
    biopac: {
      ip: {
        label: 'Server Address',
        type: 'text',
        placeholder: 'localhost or 192.168.1.100',
        required: true,
        pattern: '^[\\w.-]+$',
        errorMessage: 'Invalid server address format'
      },
      port: {
        label: 'Port',
        type: 'number',
        min: 1,
        max: 65535,
        default: 5000,
        required: true,
        errorMessage: 'Port must be between 1-65535'
      },
      channels: {
        label: 'Active Channels',
        type: 'text',
        placeholder: '1,2,3,4 or 1-8',
        required: true,
        pattern: '^(\\d+(-\\d+)?)(,\\d+(-\\d+)?)*$',
        errorMessage: 'Format: 1,2,3 or 1-4,6-8'
      },
      samplingRate: {
        label: 'Sampling Rate (Hz)',
        type: 'select',
        options: [100, 200, 500, 1000, 2000],
        default: 1000,
        required: true
      }
    },
    lsl: {
      enableOutlet: {
        label: 'Enable LSL Outlet',
        type: 'checkbox',
        default: false
      },
      streamName: {
        label: 'Stream Name',
        type: 'text',
        placeholder: 'HyperStudy_Device',
        required: false,
        default: 'HyperStudy_Bridge'
      },
      streamType: {
        label: 'Stream Type',
        type: 'select',
        options: ['Markers', 'EEG', 'fNIRS', 'Gaze', 'Audio', 'Accelerometer', 'Other'],
        default: 'Markers',
        required: false
      },
      sourceId: {
        label: 'Source ID',
        type: 'text',
        placeholder: 'hyperstudy-bridge-001',
        required: false,
        default: 'hyperstudy-bridge'
      },
      chunkSize: {
        label: 'Chunk Size',
        type: 'number',
        min: 1,
        max: 1000,
        default: 32,
        required: false,
        errorMessage: 'Chunk size must be between 1-1000'
      },
      bufferSize: {
        label: 'Buffer Size (samples)',
        type: 'number',
        min: 100,
        max: 10000,
        default: 1000,
        required: false,
        errorMessage: 'Buffer size must be between 100-10000'
      },
      enableTimestamp: {
        label: 'Include Timestamps',
        type: 'checkbox',
        default: true
      },
      enableMetadata: {
        label: 'Include Metadata',
        type: 'checkbox',
        default: true
      }
    }
  };

  // Initialize form data when device changes
  $effect(() => {
    if (device && isOpen) {
      initializeForm();
    }
  });

  function initializeForm() {
    const config = deviceConfigs[device.id];
    if (!config) return;

    const newFormData = {};
    const existingConfig = device.config || {};

    Object.entries(config).forEach(([key, fieldConfig]) => {
      if (fieldConfig.type === 'checkbox') {
        newFormData[key] = existingConfig[key] ?? fieldConfig.default ?? false;
      } else {
        newFormData[key] = existingConfig[key] ?? fieldConfig.default ?? '';
      }
    });

    formData = newFormData;

    // Initialize LSL configuration
    const lslConfigTemplate = deviceConfigs.lsl;
    const newLslConfig = {};
    const existingLslConfig = device.lslConfig || {};

    Object.entries(lslConfigTemplate).forEach(([key, fieldConfig]) => {
      if (fieldConfig.type === 'checkbox') {
        newLslConfig[key] = existingLslConfig[key] ?? fieldConfig.default ?? false;
      } else {
        newLslConfig[key] = existingLslConfig[key] ?? fieldConfig.default ?? '';
      }
    });

    lslConfig = newLslConfig;
    errors = {};
    activeTab = 'device';
  }

  function validateField(fieldName, value) {
    const config = deviceConfigs[device?.id]?.[fieldName];
    if (!config) return null;

    // Required field validation
    if (config.required && (!value || value === '')) {
      return `${config.label} is required`;
    }

    // Pattern validation
    if (config.pattern && value && !new RegExp(config.pattern).test(value)) {
      return config.errorMessage || `Invalid ${config.label} format`;
    }

    // Number validation
    if (config.type === 'number' && value !== '') {
      const num = Number(value);
      if (isNaN(num)) {
        return `${config.label} must be a number`;
      }
      if (config.min !== undefined && num < config.min) {
        return `${config.label} must be at least ${config.min}`;
      }
      if (config.max !== undefined && num > config.max) {
        return `${config.label} must be at most ${config.max}`;
      }
    }

    return null;
  }

  function validateForm() {
    const config = deviceConfigs[device?.id];
    if (!config) return false;

    const newErrors = {};
    let hasErrors = false;

    Object.keys(config).forEach(fieldName => {
      const error = validateField(fieldName, formData[fieldName]);
      if (error) {
        newErrors[fieldName] = error;
        hasErrors = true;
      }
    });

    errors = newErrors;
    return !hasErrors;
  }

  function handleFieldChange(fieldName, value, isLsl = false) {
    if (isLsl) {
      lslConfig[fieldName] = value;
    } else {
      formData[fieldName] = value;
    }

    // Clear error for this field when user starts typing
    if (errors[fieldName]) {
      const newErrors = { ...errors };
      delete newErrors[fieldName];
      errors = newErrors;
    }
  }

  async function handleSave() {
    if (!validateForm()) {
      return;
    }

    isSubmitting = true;
    try {
      // Convert form data to appropriate types
      const processedConfig = {};
      const config = deviceConfigs[device.id];

      Object.entries(formData).forEach(([key, value]) => {
        const fieldConfig = config[key];
        if (fieldConfig.type === 'number') {
          processedConfig[key] = value === '' ? fieldConfig.default : Number(value);
        } else if (fieldConfig.type === 'checkbox') {
          processedConfig[key] = Boolean(value);
        } else {
          processedConfig[key] = value;
        }
      });

      // Process LSL configuration
      const processedLslConfig = {};
      const lslConfigTemplate = deviceConfigs.lsl;

      Object.entries(lslConfig).forEach(([key, value]) => {
        const fieldConfig = lslConfigTemplate[key];
        if (fieldConfig.type === 'number') {
          processedLslConfig[key] = value === '' ? fieldConfig.default : Number(value);
        } else if (fieldConfig.type === 'checkbox') {
          processedLslConfig[key] = Boolean(value);
        } else {
          processedLslConfig[key] = value;
        }
      });

      await onSave(device.id, processedConfig, processedLslConfig);
      handleClose();
    } catch (error) {
      console.error('Failed to save configuration:', error);
      // You could add a general error state here
    } finally {
      isSubmitting = false;
    }
  }

  function handleClose() {
    formData = {};
    lslConfig = {};
    errors = {};
    isSubmitting = false;
    activeTab = 'device';
    onClose();
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      handleClose();
    } else if (e.key === 'Enter' && e.ctrlKey) {
      handleSave();
    }
  }

</script>

{#if isOpen && device}
  <div class="modal-overlay" onclick={handleClose} onkeydown={handleKeydown}>
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <div class="modal-header">
        <h2>Configure {device.name}</h2>
        <button class="close-btn" onclick={handleClose}>×</button>
      </div>

      <div class="modal-body">
        <p class="instructions">
          Configure settings for your {device.name}. Use <kbd>Ctrl+Enter</kbd> to save quickly.
        </p>

        <!-- Tab Navigation -->
        <div class="tab-nav">
          <button
            class="tab-btn"
            class:active={activeTab === 'device'}
            onclick={() => activeTab = 'device'}
          >
            Device Settings
          </button>
          <button
            class="tab-btn"
            class:active={activeTab === 'lsl'}
            onclick={() => activeTab = 'lsl'}
          >
            LSL Configuration
          </button>
        </div>

        <form class="config-form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
          {#if activeTab === 'device'}
            <!-- Device Configuration Tab -->
            {#if deviceConfigs[device.id]}
              {#each Object.entries(deviceConfigs[device.id]) as [fieldName, fieldConfig]}
                <div class="form-group">
                  <label for={fieldName} class="form-label">
                    {fieldConfig.label}
                    {#if fieldConfig.required}
                      <span class="required">*</span>
                    {/if}
                  </label>

                  {#if fieldConfig.type === 'select'}
                    <select
                      id={fieldName}
                      class="form-input"
                      class:error={errors[fieldName]}
                      value={formData[fieldName]}
                      onchange={(e) => handleFieldChange(fieldName, e.target.value)}
                    >
                      <option value="">Select {fieldConfig.label}</option>
                      {#each fieldConfig.options as option}
                        <option value={option}>{option}</option>
                      {/each}
                    </select>
                  {:else if fieldConfig.type === 'checkbox'}
                    <label class="checkbox-wrapper">
                      <input
                        id={fieldName}
                        type="checkbox"
                        class="form-checkbox"
                        checked={formData[fieldName]}
                        onchange={(e) => handleFieldChange(fieldName, e.target.checked)}
                      />
                      <span class="checkbox-label">Enable {fieldConfig.label}</span>
                    </label>
                  {:else}
                    <input
                      id={fieldName}
                      type={fieldConfig.type}
                      class="form-input"
                      class:error={errors[fieldName]}
                      placeholder={fieldConfig.placeholder || ''}
                      min={fieldConfig.min}
                      max={fieldConfig.max}
                      value={formData[fieldName]}
                      oninput={(e) => handleFieldChange(fieldName, e.target.value)}
                    />
                  {/if}

                  {#if errors[fieldName]}
                    <div class="form-error">{errors[fieldName]}</div>
                  {/if}
                </div>
              {/each}
            {:else}
              <div class="no-config">
                <p>No configuration options available for this device type.</p>
              </div>
            {/if}
          {:else if activeTab === 'lsl'}
            <!-- LSL Configuration Tab -->
            <div class="lsl-config-section">
              <p class="section-description">
                Configure Lab Streaming Layer (LSL) output for this device. When enabled,
                device data will be published as an LSL stream for use by other applications.
              </p>

              {#each Object.entries(deviceConfigs.lsl) as [fieldName, fieldConfig]}
                <div class="form-group">
                  {#if fieldConfig.type === 'checkbox'}
                    <label class="checkbox-wrapper">
                      <input
                        id="lsl-{fieldName}"
                        type="checkbox"
                        class="form-checkbox"
                        checked={lslConfig[fieldName]}
                        onchange={(e) => handleFieldChange(fieldName, e.target.checked, true)}
                      />
                      <span class="checkbox-label">
                        {fieldConfig.label}
                        {#if fieldConfig.required}
                          <span class="required">*</span>
                        {/if}
                      </span>
                    </label>
                  {:else}
                    <label for="lsl-{fieldName}" class="form-label">
                      {fieldConfig.label}
                      {#if fieldConfig.required}
                        <span class="required">*</span>
                      {/if}
                    </label>

                    {#if fieldConfig.type === 'select'}
                      <select
                        id="lsl-{fieldName}"
                        class="form-input"
                        class:error={errors[fieldName]}
                        value={lslConfig[fieldName]}
                        onchange={(e) => handleFieldChange(fieldName, e.target.value, true)}
                      >
                        <option value="">Select {fieldConfig.label}</option>
                        {#each fieldConfig.options as option}
                          <option value={option}>{option}</option>
                        {/each}
                      </select>
                    {:else}
                      <input
                        id="lsl-{fieldName}"
                        type={fieldConfig.type}
                        class="form-input"
                        class:error={errors[fieldName]}
                        placeholder={fieldConfig.placeholder || ''}
                        min={fieldConfig.min}
                        max={fieldConfig.max}
                        value={lslConfig[fieldName]}
                        oninput={(e) => handleFieldChange(fieldName, e.target.value, true)}
                        disabled={!lslConfig.enableOutlet && fieldName !== 'enableOutlet'}
                      />
                    {/if}
                  {/if}

                  {#if errors[fieldName]}
                    <div class="form-error">{errors[fieldName]}</div>
                  {/if}
                </div>
              {/each}

              {#if lslConfig.enableOutlet}
                <div class="lsl-info">
                  <h4>Stream Preview</h4>
                  <div class="stream-preview">
                    <div class="preview-item">
                      <strong>Name:</strong> {lslConfig.streamName || 'HyperStudy_Bridge'}
                    </div>
                    <div class="preview-item">
                      <strong>Type:</strong> {lslConfig.streamType || 'Markers'}
                    </div>
                    <div class="preview-item">
                      <strong>Source ID:</strong> {lslConfig.sourceId || 'hyperstudy-bridge'}
                    </div>
                    <div class="preview-item">
                      <strong>Buffer Size:</strong> {lslConfig.bufferSize || 1000} samples
                    </div>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        </form>
      </div>

      <div class="modal-footer">
        <button class="cancel-btn" onclick={handleClose} disabled={isSubmitting}>
          Cancel
        </button>
        <button
          class="save-btn"
          onclick={handleSave}
          disabled={isSubmitting || Object.keys(errors).length > 0}
        >
          {isSubmitting ? 'Saving...' : 'Save Configuration'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .modal {
    background: var(--color-surface);
    border-radius: 12px;
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    animation: slideUp 0.3s ease;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem;
    border-bottom: 1px solid var(--color-border);
  }

  .modal-header h2 {
    margin: 0;
    color: var(--color-primary);
    font-size: 1.5rem;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 2rem;
    color: var(--color-text-secondary);
    cursor: pointer;
    padding: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-text-primary);
  }

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
  }

  .instructions {
    color: var(--color-text-secondary);
    margin-bottom: 1.5rem;
    font-size: 0.9rem;
  }

  kbd {
    background: rgba(255, 255, 255, 0.1);
    padding: 2px 6px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 0.85em;
  }

  .config-form {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .form-label {
    font-weight: 500;
    color: var(--color-text-primary);
    font-size: 0.9rem;
  }

  .required {
    color: var(--color-error);
    margin-left: 0.25rem;
  }

  .form-input, select.form-input {
    padding: 0.75rem;
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    color: var(--color-text-primary);
    font-size: 1rem;
    transition: all 0.2s;
  }

  .form-input:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 3px rgba(76, 175, 80, 0.1);
  }

  .form-input.error {
    border-color: var(--color-error);
    box-shadow: 0 0 0 3px rgba(239, 68, 68, 0.1);
  }

  .form-input::placeholder {
    color: var(--color-text-secondary);
  }

  .checkbox-wrapper {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    cursor: pointer;
    padding: 0.5rem 0;
  }

  .form-checkbox {
    width: 18px;
    height: 18px;
    accent-color: var(--color-primary);
  }

  .checkbox-label {
    color: var(--color-text-primary);
    font-size: 0.95rem;
  }

  .form-error {
    color: var(--color-error);
    font-size: 0.85rem;
    margin-top: 0.25rem;
  }

  .no-config {
    text-align: center;
    padding: 2rem;
    color: var(--color-text-secondary);
    background: var(--color-background);
    border-radius: 8px;
    border: 1px solid var(--color-border);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 1rem;
    padding: 1.5rem;
    border-top: 1px solid var(--color-border);
  }

  .cancel-btn,
  .save-btn {
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 8px;
    font-size: 1rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .cancel-btn {
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .cancel-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.05);
    color: var(--color-text-primary);
  }

  .save-btn {
    background: var(--color-primary);
    color: white;
  }

  .save-btn:hover:not(:disabled) {
    background: var(--color-primary-hover);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(76, 175, 80, 0.3);
  }

  .save-btn:disabled,
  .cancel-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Tab Navigation */
  .tab-nav {
    display: flex;
    border-bottom: 1px solid var(--color-border);
    margin-bottom: 1.5rem;
  }

  .tab-btn {
    padding: 0.75rem 1.5rem;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    border-bottom: 3px solid transparent;
  }

  .tab-btn:hover {
    color: var(--color-text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  .tab-btn.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
  }

  /* LSL Configuration Section */
  .lsl-config-section {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .section-description {
    color: var(--color-text-secondary);
    font-size: 0.9rem;
    margin: 0;
    padding: 1rem;
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
  }

  .lsl-info {
    margin-top: 1rem;
    padding: 1rem;
    background: var(--color-background);
    border: 1px solid var(--color-border);
    border-radius: 8px;
  }

  .lsl-info h4 {
    margin: 0 0 0.75rem 0;
    color: var(--color-text-primary);
    font-size: 1rem;
    font-weight: 600;
  }

  .stream-preview {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .preview-item {
    display: flex;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: var(--color-text-secondary);
  }

  .preview-item strong {
    color: var(--color-text-primary);
    min-width: 100px;
  }

  /* Custom select styling */
  select.form-input {
    background-image: url("data:image/svg+xml;charset=US-ASCII,%3csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 4 5'%3e%3cpath fill='%23ffffff80' d='m2 0-2 2h4zm0 5 2-2h-4z'/%3e%3c/svg%3e");
    background-repeat: no-repeat;
    background-position: right 0.75rem center;
    background-size: 16px;
    padding-right: 2.5rem;
    appearance: none;
    cursor: pointer;
  }
</style>