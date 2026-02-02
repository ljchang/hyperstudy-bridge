import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import '@testing-library/jest-dom';
import DeviceConfigModal from '../DeviceConfigModal.svelte';

// Mock the bridge store
vi.mock('../../stores/websocket.svelte.js', () => ({
  connectDevice: vi.fn(),
  disconnectDevice: vi.fn(),
  sendCommand: vi.fn(),
}));

describe('DeviceConfigModal', () => {
  const user = userEvent.setup();

  // TTL device - component only has 'port' field (Serial Port)
  const mockTtlDevice = {
    id: 'ttl',
    name: 'TTL Pulse Generator',
    type: 'Adafruit RP2040',
    config: {
      port: '/dev/ttyUSB0'
    }
  };

  const mockKernelDevice = {
    id: 'kernel',
    name: 'Kernel Flow2',
    type: 'fNIRS',
    config: {
      ip: '127.0.0.1',
      port: 6767,
      samplingRate: 10
    }
  };

  const mockPupilDevice = {
    id: 'pupil',
    name: 'Pupil Labs Neon',
    type: 'Eye Tracker',
    config: {
      url: 'localhost:8081',
      streamGaze: true,
      streamVideo: false,
      gazeFormat: 'normalized'
    }
  };

  const mockProps = {
    isOpen: true,
    device: mockTtlDevice,
    onSave: vi.fn(),
    onClose: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Modal Visibility', () => {
    it('renders when isOpen is true', () => {
      render(DeviceConfigModal, mockProps);

      expect(screen.getByText('Configure TTL Pulse Generator')).toBeInTheDocument();
    });

    it('does not render when isOpen is false', () => {
      render(DeviceConfigModal, { ...mockProps, isOpen: false });

      expect(screen.queryByText('Configure TTL Pulse Generator')).not.toBeInTheDocument();
    });

    it('handles null device gracefully', () => {
      render(DeviceConfigModal, { ...mockProps, device: null });

      expect(screen.queryByText(/Configure/)).not.toBeInTheDocument();
    });
  });

  describe('TTL Device Configuration', () => {
    it('displays Serial Port field', () => {
      render(DeviceConfigModal, mockProps);

      expect(screen.getByText('Serial Port')).toBeInTheDocument();
    });

    it('validates TTL port format', async () => {
      render(DeviceConfigModal, mockProps);

      const portInput = screen.getByDisplayValue('/dev/ttyUSB0');
      await user.clear(portInput);
      await user.type(portInput, 'invalid-port');

      const saveButton = screen.getByText('Save Configuration');
      await user.click(saveButton);

      await waitFor(() => {
        expect(screen.getByText(/Invalid port format/)).toBeInTheDocument();
      });
    });

    it('validates required TTL fields', async () => {
      render(DeviceConfigModal, mockProps);

      const portInput = screen.getByDisplayValue('/dev/ttyUSB0');
      await user.clear(portInput);

      const saveButton = screen.getByText('Save Configuration');
      await user.click(saveButton);

      await waitFor(() => {
        expect(screen.getByText(/Serial Port is required/)).toBeInTheDocument();
      });
    });
  });

  describe('Kernel Device Configuration', () => {
    it('displays Kernel-specific configuration fields', () => {
      render(DeviceConfigModal, { ...mockProps, device: mockKernelDevice });

      expect(screen.getByText('IP Address')).toBeInTheDocument();
      expect(screen.getByText('Port')).toBeInTheDocument();
      expect(screen.getByText('Sampling Rate (Hz)')).toBeInTheDocument();
    });

    it('validates IP address format', async () => {
      render(DeviceConfigModal, { ...mockProps, device: mockKernelDevice });

      const ipInput = screen.getByDisplayValue('127.0.0.1');
      await user.clear(ipInput);
      await user.type(ipInput, '999.999.999.999');

      const saveButton = screen.getByText('Save Configuration');
      await user.click(saveButton);

      await waitFor(() => {
        expect(screen.getByText(/Invalid IP address format/)).toBeInTheDocument();
      });
    });
  });

  describe('Pupil Device Configuration', () => {
    it('displays Pupil-specific configuration fields', () => {
      render(DeviceConfigModal, { ...mockProps, device: mockPupilDevice });

      expect(screen.getByText('Device URL')).toBeInTheDocument();
      expect(screen.getByText('Stream Gaze Data')).toBeInTheDocument();
      expect(screen.getByText('Stream Video')).toBeInTheDocument();
      expect(screen.getByText('Gaze Data Format')).toBeInTheDocument();
    });

    it('handles checkbox controls correctly', async () => {
      render(DeviceConfigModal, { ...mockProps, device: mockPupilDevice });

      const gazeCheckbox = screen.getByLabelText('Stream Gaze Data');
      const videoCheckbox = screen.getByLabelText('Stream Video');

      expect(gazeCheckbox).toBeChecked();
      expect(videoCheckbox).not.toBeChecked();

      await user.click(videoCheckbox);
      expect(videoCheckbox).toBeChecked();
    });

    it('validates Pupil URL format', async () => {
      render(DeviceConfigModal, { ...mockProps, device: mockPupilDevice });

      const urlInput = screen.getByDisplayValue('localhost:8081');
      await user.clear(urlInput);
      await user.type(urlInput, 'invalid url with spaces');

      const saveButton = screen.getByText('Save Configuration');
      await user.click(saveButton);

      await waitFor(() => {
        expect(screen.getByText(/Invalid URL format/)).toBeInTheDocument();
      });
    });
  });

  describe('Modal Actions', () => {
    it('calls onClose when Cancel button is clicked', async () => {
      render(DeviceConfigModal, mockProps);

      const cancelButton = screen.getByText('Cancel');
      await user.click(cancelButton);

      expect(mockProps.onClose).toHaveBeenCalled();
    });

    it('calls onClose when close button (×) is clicked', async () => {
      render(DeviceConfigModal, mockProps);

      const closeButton = screen.getByText('×');
      await user.click(closeButton);

      expect(mockProps.onClose).toHaveBeenCalled();
    });

    it('calls onClose when clicking outside modal', async () => {
      render(DeviceConfigModal, mockProps);

      const overlay = document.querySelector('.modal-overlay');
      await user.click(overlay);

      expect(mockProps.onClose).toHaveBeenCalled();
    });

    it('does not close when clicking inside modal', async () => {
      render(DeviceConfigModal, mockProps);

      const modal = document.querySelector('.modal');
      await user.click(modal);

      expect(mockProps.onClose).not.toHaveBeenCalled();
    });
  });

  describe('Form Submission', () => {
    it('disables save button while submitting', async () => {
      mockProps.onSave.mockImplementation(() => new Promise(resolve => setTimeout(resolve, 100)));
      render(DeviceConfigModal, mockProps);

      const saveButton = screen.getByText('Save Configuration');
      await user.click(saveButton);

      expect(saveButton).toBeDisabled();
      expect(screen.getByText('Saving...')).toBeInTheDocument();
    });
  });

  describe('Error Handling', () => {
    it('handles invalid device configurations gracefully', () => {
      const invalidDevice = {
        id: 'unknown',
        name: 'Unknown Device',
        config: {}
      };

      render(DeviceConfigModal, { ...mockProps, device: invalidDevice });

      // Should not crash, might show a fallback message
      expect(screen.queryByText(/Configure Unknown Device/)).toBeInTheDocument();
    });

    it('handles missing device configuration fields', () => {
      const deviceWithoutConfig = {
        ...mockTtlDevice,
        config: {}
      };

      render(DeviceConfigModal, { ...mockProps, device: deviceWithoutConfig });

      // Should use default values
      expect(screen.getByDisplayValue('')).toBeInTheDocument(); // Empty port field
    });
  });
});
