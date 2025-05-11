import { pgTable, text, serial, integer, boolean } from "drizzle-orm/pg-core";
import { createInsertSchema } from "drizzle-zod";
import { z } from "zod";

// Network Configuration Schema
export const networkConfig = pgTable("network_config", {
  id: serial("id").primaryKey(),
  ipConfigType: text("ip_config_type").notNull().default("dhcp"), // 'dhcp' or 'static'
  ipAddress: text("ip_address"),
  subnetMask: text("subnet_mask"),
  gateway: text("gateway"),
  macAddress: text("mac_address").notNull(),
});

export const insertNetworkConfigSchema = createInsertSchema(networkConfig).omit({
  id: true,
});

// ArtNet Configuration Schema
export const artnetConfig = pgTable("artnet_config", {
  id: serial("id").primaryKey(),
  net: integer("net").notNull().default(0), // 0-127
  subnet: integer("subnet").notNull().default(0), // 0-15
  deviceName: text("device_name").notNull().default("ArtNet Node"),
  protocolVersion: text("protocol_version").notNull().default("ArtNet 3"), // 'ArtNet 3' or 'ArtNet 4'
});

export const insertArtnetConfigSchema = createInsertSchema(artnetConfig).omit({
  id: true,
});

// DMX Port Configuration Schema
export const dmxPortConfig = pgTable("dmx_port_config", {
  id: serial("id").primaryKey(),
  portNumber: integer("port_number").notNull(), // 1-4
  mode: text("mode").notNull().default("on"), // 'on', 'off', or 'blackout'
  universe: integer("universe").notNull().default(0), // 0-15
  mergeMode: text("merge_mode").notNull().default("htp"), // 'htp' or 'ltp'
  outputRate: text("output_rate").notNull().default("normal"), // 'slow', 'normal', 'fast', 'max'
  packetsPerSecond: integer("packets_per_second").notNull().default(0),
});

export const insertDmxPortConfigSchema = createInsertSchema(dmxPortConfig).omit({
  id: true,
});

// System Information Schema
export const systemInfo = pgTable("system_info", {
  id: serial("id").primaryKey(),
  firmwareVersion: text("firmware_version").notNull(),
  hardwareVersion: text("hardware_version").notNull(),
  uptime: text("uptime").notNull(),
  temperature: text("temperature").notNull(),
  memoryUsage: integer("memory_usage").notNull(), // percentage
  cpuLoad: integer("cpu_load").notNull(), // percentage
  artnetTraffic: integer("artnet_traffic").notNull(), // packets per second
  systemStatus: text("system_status").notNull().default("Running"),
  deviceId: text("device_id").notNull(),
});

export const insertSystemInfoSchema = createInsertSchema(systemInfo).omit({
  id: true,
});

// Type definitions
export type NetworkConfig = typeof networkConfig.$inferSelect;
export type InsertNetworkConfig = z.infer<typeof insertNetworkConfigSchema>;

export type ArtnetConfig = typeof artnetConfig.$inferSelect;
export type InsertArtnetConfig = z.infer<typeof insertArtnetConfigSchema>;

export type DmxPortConfig = typeof dmxPortConfig.$inferSelect;
export type InsertDmxPortConfig = z.infer<typeof insertDmxPortConfigSchema>;

export type SystemInfo = typeof systemInfo.$inferSelect;
export type InsertSystemInfo = z.infer<typeof insertSystemInfoSchema>;
