import { useState, useEffect } from "react";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { isValidIpAddress } from "@/lib/validators";
import { NetworkConfigUpdateItem } from "@shared/types";

interface NetworkFormValues {
  ipConfigType: "dhcp" | "static";
  ipAddress?: number[];
  subnetMask?: number[];
  gateway?: number[];
}

interface NetworkSettingsProps {
  data: any;
  onSave: (data: any) => void;
}

// Helper function to convert IP array to string
const ipArrayToString = (ip: number[] | undefined): string => {
  if (!ip) return "";
  return ip.join(".");
};

// Format MAC address: backend sends [u8; 6] as number[], display as XX:XX:XX:XX:XX:XX
const macAddressToString = (mac: number[] | string | undefined): string => {
  if (mac == null) return "";
  if (typeof mac === "string") return mac;
  if (!Array.isArray(mac) || mac.length !== 6) return "";
  return mac.map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(":");
};

// Helper function to convert IP string to array
const ipStringToArray = (ip: string): number[] | undefined => {
  if (!ip) return undefined;
  const parts = ip.split(".").map(Number);
  if (parts.length !== 4 || parts.some(isNaN)) return undefined;
  return parts;
};

export default function NetworkSettings({ data, onSave }: NetworkSettingsProps) {
  const [isStatic, setIsStatic] = useState(false);
  const [formData, setFormData] = useState<NetworkFormValues>({
      ipConfigType: "dhcp",
    ipAddress: undefined,
    subnetMask: undefined,
    gateway: undefined,
  });
  const [rawValues, setRawValues] = useState<Record<string, string>>({
      ipAddress: "",
      subnetMask: "",
      gateway: "",
  });
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isDirty, setIsDirty] = useState(false);

  useEffect(() => {
    if (data?.networkConfig && !isDirty) {
      setFormData({
        ipConfigType: data.networkConfig.ipConfigType,
        ipAddress: data.networkConfig.ipAddress,
        subnetMask: data.networkConfig.subnetMask,
        gateway: data.networkConfig.gateway,
      });
      setRawValues({
        ipAddress: ipArrayToString(data.networkConfig.ipAddress),
        subnetMask: ipArrayToString(data.networkConfig.subnetMask),
        gateway: ipArrayToString(data.networkConfig.gateway),
      });
      setIsStatic(data.networkConfig.ipConfigType === "static");
    }
  }, [data, isDirty]);

  useEffect(() => {
    setIsStatic(formData.ipConfigType === "static");
  }, [formData.ipConfigType]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, value } = e.target;
    setRawValues(prev => ({ ...prev, [name]: value }));
    setIsDirty(true);
    
    const ipArray = ipStringToArray(value);
    if (ipArray) {
      setFormData(prev => ({ ...prev, [name]: ipArray }));
    } else {
      setFormData(prev => ({ ...prev, [name]: undefined }));
    }
    if (errors[name]) {
      setErrors(prev => ({ ...prev, [name]: "" }));
    }
  };

  const handleRadioChange = (value: string) => {
    setFormData(prev => ({ ...prev, ipConfigType: value as "dhcp" | "static" }));
    setIsDirty(true);
  };

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const newErrors: Record<string, string> = {};
    let valid = true;

    if (formData.ipConfigType === "static") {
      if (!formData.ipAddress || !isValidIpAddress(ipArrayToString(formData.ipAddress))) {
        newErrors.ipAddress = "Please enter a valid IP address";
        valid = false;
      }
      if (!formData.subnetMask || !isValidIpAddress(ipArrayToString(formData.subnetMask))) {
        newErrors.subnetMask = "Please enter a valid subnet mask";
        valid = false;
      }
      if (!formData.gateway || !isValidIpAddress(ipArrayToString(formData.gateway))) {
        newErrors.gateway = "Please enter a valid gateway address";
        valid = false;
      }
    } else {
      // For DHCP, only validate if values are provided (as fallback)
      if (formData.ipAddress && !isValidIpAddress(ipArrayToString(formData.ipAddress))) {
        newErrors.ipAddress = "Please enter a valid fallback IP address";
        valid = false;
      }
      if (formData.subnetMask && !isValidIpAddress(ipArrayToString(formData.subnetMask))) {
        newErrors.subnetMask = "Please enter a valid fallback subnet mask";
        valid = false;
      }
      if (formData.gateway && !isValidIpAddress(ipArrayToString(formData.gateway))) {
        newErrors.gateway = "Please enter a valid fallback gateway address";
        valid = false;
      }
    }

    setErrors(newErrors);
    if (!valid) return;

    // Create the network config update object (editable fields only)
    const networkConfigUpdate: NetworkConfigUpdateItem = {
      ipConfigType: formData.ipConfigType,
      ipAddress: formData.ipAddress,
      subnetMask: formData.subnetMask,
      gateway: formData.gateway,
      // Note: macAddress and current_* fields are intentionally omitted - they're read-only
    };

    onSave({
      type: "networkConfigUpdate",
      data: networkConfigUpdate
    });
    setIsDirty(false);
  };

  return (
    <Card>
      <CardContent className="pt-6">
        <h2 className="text-lg font-medium text-gray-800 mb-6">Network Configuration</h2>
        
        <form onSubmit={onSubmit} className="space-y-6">
          {/* IP Configuration Type */}
          <div className="space-y-1">
            <Label className="block text-sm font-medium text-gray-700">IP Configuration</Label>
            <RadioGroup 
              value={formData.ipConfigType}
              onValueChange={handleRadioChange}
              className="flex items-center space-x-4 mt-2"
            >
              <div className="flex items-center">
                <RadioGroupItem value="dhcp" id="dhcp" />
                <Label htmlFor="dhcp" className="ml-2 text-sm text-gray-700">DHCP</Label>
              </div>
              <div className="flex items-center">
                <RadioGroupItem value="static" id="static" />
                <Label htmlFor="static" className="ml-2 text-sm text-gray-700">Static IP</Label>
              </div>
            </RadioGroup>
          </div>
          
          {/* IP Settings section - shown for both DHCP and static */}
          <div className="space-y-4">
            {/* IP Address */}
            <div>
              <Label htmlFor="ipAddress" className="flex items-center text-sm font-medium text-gray-700">
                {isStatic ? "IP Address" : "Fallback IP Address"}
                {!isStatic && (
                  <span className="ml-2 text-xs text-gray-500">(Used when DHCP server is unavailable)</span>
                )}
              </Label>
              <Input
                id="ipAddress"
                name="ipAddress"
                value={rawValues.ipAddress}
                onChange={handleChange}
                placeholder="192.168.1.100"
                className={`mt-1 ${errors.ipAddress ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
              />
              {errors.ipAddress && (
                <p className="mt-1 text-sm text-red-600">{errors.ipAddress}</p>
              )}
            </div>
            
            {/* Subnet Mask */}
            <div>
              <Label htmlFor="subnetMask" className="flex items-center text-sm font-medium text-gray-700">
                {isStatic ? "Subnet Mask" : "Fallback Subnet Mask"}
                {!isStatic && (
                  <span className="ml-2 text-xs text-gray-500">(Used when DHCP server is unavailable)</span>
                )}
              </Label>
              <Input
                id="subnetMask"
                name="subnetMask"
                value={rawValues.subnetMask}
                onChange={handleChange}
                placeholder="255.255.255.0"
                className={`mt-1 ${errors.subnetMask ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
              />
              {errors.subnetMask && (
                <p className="mt-1 text-sm text-red-600">{errors.subnetMask}</p>
              )}
            </div>
            
            {/* Gateway - only shown for static IP */}
            {isStatic && (
              <div>
                <Label htmlFor="gateway" className="block text-sm font-medium text-gray-700">Gateway</Label>
                <Input
                  id="gateway"
                  name="gateway"
                  value={rawValues.gateway}
                  onChange={handleChange}
                  placeholder="192.168.1.1"
                  className={`mt-1 ${errors.gateway ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
                />
                {errors.gateway && (
                  <p className="mt-1 text-sm text-red-600">{errors.gateway}</p>
                )}
              </div>
            )}
          </div>
          
          {/* Current Network Info */}
          <div className="pt-4 border-t border-gray-200">
            <h3 className="text-sm font-medium text-gray-700 mb-3">Current Network Information</h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-gray-500">IP Address:</span>
                <span className="ml-2 text-gray-900">{ipArrayToString(data?.networkConfig?.currentIpAddress) || "192.168.1.120"}</span>
              </div>
              <div>
                <span className="text-gray-500">Subnet Mask:</span>
                <span className="ml-2 text-gray-900">{ipArrayToString(data?.networkConfig?.currentSubnetMask) || "255.255.255.0"}</span>
              </div>
              <div>
                <span className="text-gray-500">Gateway:</span>
                <span className="ml-2 text-gray-900">{ipArrayToString(data?.networkConfig?.currentGateway) || "192.168.1.1"}</span>
              </div>
              <div>
                <span className="text-gray-500">MAC Address:</span>
                <span className="ml-2 text-gray-900">{macAddressToString(data?.networkConfig?.macAddress) || "—"}</span>
              </div>
            </div>
          </div>
          
          {/* Submit Button */}
          <div className="flex justify-end">
            <Button type="submit" className="bg-blue-600 hover:bg-blue-700">
              Save Network Settings
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
