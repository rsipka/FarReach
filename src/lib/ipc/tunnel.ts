import { invoke } from '@tauri-apps/api/core';

export interface TunnelConfig {
  id: string;
  tunnel_type: 'Local' | 'Dynamic';
  local_port: number;
  remote_host: string;
  remote_port: number;
  session_id: string;
  status: 'stopped' | 'starting' | 'running';
}

export async function tunnelCreate(
  tunnelType: TunnelConfig['tunnel_type'],
  localPort: number,
  remoteHost: string,
  remotePort: number,
  sessionId: string
): Promise<TunnelConfig> {
  return invoke<TunnelConfig>('tunnel_create', { tunnelType, localPort, remoteHost, remotePort, sessionId });
}

export async function tunnelStart(tunnelId: string): Promise<void> {
  return invoke('tunnel_start', { tunnelId });
}

export async function tunnelStop(tunnelId: string): Promise<void> {
  return invoke('tunnel_stop', { tunnelId });
}

export async function tunnelRemove(tunnelId: string): Promise<void> {
  return invoke('tunnel_remove', { tunnelId });
}

export async function tunnelList(): Promise<TunnelConfig[]> {
  return invoke<TunnelConfig[]>('tunnel_list');
}
