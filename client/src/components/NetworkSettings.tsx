import { useState, useEffect } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { z } from "zod";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { isValidIpAddress } from "@/lib/validators";

const networkSchema = z.object({
  ipConfigType: z.enum(["dhcp", "static"]),
  ipAddress: z.string().refine(val => val === '' || isValidIpAddress(val), {
    message: "Please enter a valid IP address",
  }).optional(),
  subnetMask: z.string().refine(val => val === '' || isValidIpAddress(val), {
    message: "Please enter a valid subnet mask",
  }).optional(),
  gateway: z.string().refine(val => val === '' || isValidIpAddress(val), {
    message: "Please enter a valid gateway address",
  }).optional(),
});

interface NetworkSettingsProps {
  data: any;
  onSave: (data: any) => void;
}

export default function NetworkSettings({ data, onSave }: NetworkSettingsProps) {
  const [isStatic, setIsStatic] = useState(false);
  
  const { register, handleSubmit, formState: { errors }, setValue, watch } = useForm({
    resolver: zodResolver(networkSchema),
    defaultValues: {
      ipConfigType: "dhcp",
      ipAddress: "",
      subnetMask: "",
      gateway: "",
    },
  });

  const ipConfigType = watch("ipConfigType");

  useEffect(() => {
    if (data?.networkConfig) {
      setValue("ipConfigType", data.networkConfig.ipConfigType);
      setValue("ipAddress", data.networkConfig.ipAddress || "");
      setValue("subnetMask", data.networkConfig.subnetMask || "");
      setValue("gateway", data.networkConfig.gateway || "");
      setIsStatic(data.networkConfig.ipConfigType === "static");
    }
  }, [data, setValue]);

  useEffect(() => {
    setIsStatic(ipConfigType === "static");
  }, [ipConfigType]);

  const onSubmit = (formData: any) => {
    onSave(formData);
  };

  return (
    <Card>
      <CardContent className="pt-6">
        <h2 className="text-lg font-medium text-gray-800 mb-6">Network Configuration</h2>
        
        <form onSubmit={handleSubmit(onSubmit)} className="space-y-6">
          {/* IP Configuration Type */}
          <div className="space-y-1">
            <Label className="block text-sm font-medium text-gray-700">IP Configuration</Label>
            <RadioGroup 
              defaultValue="dhcp" 
              className="flex items-center space-x-4 mt-2"
              {...register("ipConfigType")}
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
          
          {/* Static IP Settings */}
          {isStatic && (
            <div className="space-y-4">
              {/* IP Address */}
              <div>
                <Label htmlFor="ipAddress" className="block text-sm font-medium text-gray-700">IP Address</Label>
                <Input
                  id="ipAddress"
                  placeholder="192.168.1.100"
                  className={`mt-1 ${errors.ipAddress ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
                  {...register("ipAddress")}
                />
                {errors.ipAddress && (
                  <p className="mt-1 text-sm text-red-600">{errors.ipAddress.message as string}</p>
                )}
              </div>
              
              {/* Subnet Mask */}
              <div>
                <Label htmlFor="subnetMask" className="block text-sm font-medium text-gray-700">Subnet Mask</Label>
                <Input
                  id="subnetMask"
                  placeholder="255.255.255.0"
                  className={`mt-1 ${errors.subnetMask ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
                  {...register("subnetMask")}
                />
                {errors.subnetMask && (
                  <p className="mt-1 text-sm text-red-600">{errors.subnetMask.message as string}</p>
                )}
              </div>
              
              {/* Gateway */}
              <div>
                <Label htmlFor="gateway" className="block text-sm font-medium text-gray-700">Gateway</Label>
                <Input
                  id="gateway"
                  placeholder="192.168.1.1"
                  className={`mt-1 ${errors.gateway ? 'border-red-300 focus:ring-red-500 focus:border-red-500' : ''}`}
                  {...register("gateway")}
                />
                {errors.gateway && (
                  <p className="mt-1 text-sm text-red-600">{errors.gateway.message as string}</p>
                )}
              </div>
            </div>
          )}
          
          {/* Current Network Info */}
          <div className="pt-4 border-t border-gray-200">
            <h3 className="text-sm font-medium text-gray-700 mb-3">Current Network Information</h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-gray-500">IP Address:</span>
                <span className="ml-2 text-gray-900">{data?.networkConfig?.currentIpAddress || "192.168.1.120"}</span>
              </div>
              <div>
                <span className="text-gray-500">Subnet Mask:</span>
                <span className="ml-2 text-gray-900">{data?.networkConfig?.currentSubnetMask || "255.255.255.0"}</span>
              </div>
              <div>
                <span className="text-gray-500">Gateway:</span>
                <span className="ml-2 text-gray-900">{data?.networkConfig?.currentGateway || "192.168.1.1"}</span>
              </div>
              <div>
                <span className="text-gray-500">MAC Address:</span>
                <span className="ml-2 text-gray-900">{data?.networkConfig?.macAddress || "F8:4D:89:7C:0B:A2"}</span>
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
