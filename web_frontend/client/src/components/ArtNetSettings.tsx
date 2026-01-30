import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { ArtnetConfigUpdateItem } from "@shared/types";

interface ArtnetFormValues {
  net: string;
  subnet: string;
  deviceName: string;
}

interface ArtNetSettingsProps {
  data: any;
  onSave: (data: any) => void;
}

export default function ArtNetSettings({ data, onSave }: ArtNetSettingsProps) {
  const [formData, setFormData] = useState<ArtnetFormValues>({
      net: "0",
      subnet: "0",
      deviceName: "OpenLumen Node",
  });
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [isDirty, setIsDirty] = useState(false);

  // Initialize form data when component mounts
  useEffect(() => {
    if (data?.artnetConfig) {
      setFormData({
        net: String(data.artnetConfig.net ?? 0),
        subnet: String(data.artnetConfig.subnet ?? 0),
        deviceName: data.artnetConfig.deviceName || "OpenLumen Node",
      });
    }
  }, []); // Only run on mount

  // Update form data when backend sends new data and form is not dirty
  useEffect(() => {
    if (data?.artnetConfig && !isDirty) {
      setFormData(prev => ({
        ...prev,
        net: String(data.artnetConfig.net ?? 0),
        subnet: String(data.artnetConfig.subnet ?? 0),
        deviceName: data.artnetConfig.deviceName || prev.deviceName,
      }));
    }
  }, [data, isDirty]);

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({ ...prev, [name]: value }));
    setIsDirty(true);
    if (errors[name]) {
      setErrors(prev => ({ ...prev, [name]: "" }));
    }
  };

  const handleSelectChange = (name: string, value: string) => {
    setFormData(prev => ({ ...prev, [name]: value }));
    setIsDirty(true);
  };

  const handleRadioChange = (value: string) => {
    setFormData(prev => ({ ...prev, protocolVersion: value as "ArtNet 3" | "ArtNet 4" }));
    setIsDirty(true);
  };

  const onSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const newErrors: Record<string, string> = {};

    if (formData.deviceName.length > 17) {
      newErrors.deviceName = "Device name cannot exceed 17 characters";
      setErrors(newErrors);
      return;
    }

    // Create the ArtNet config update object (editable fields only)
    const artnetConfigUpdate: ArtnetConfigUpdateItem = {
      net: Number(formData.net),
      subnet: Number(formData.subnet),
      deviceName: formData.deviceName,
      // Note: id is intentionally omitted - it's read-only
    };

    onSave({
      type: "artnetConfigUpdate",
      data: artnetConfigUpdate
    });
    setIsDirty(false);
  };

  return (
    <Card className="border-border bg-card">
      <CardContent className="pt-6">
        <h2 className="text-lg font-medium text-foreground mb-6">ArtNet Configuration</h2>

        <form onSubmit={onSubmit} className="space-y-6">
          <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
            <div>
              <Label className="text-sm font-medium text-foreground">Net</Label>
              <Select 
                value={formData.net}
                onValueChange={(value) => handleSelectChange("net", value)}
              >
                        <SelectTrigger>
                  <SelectValue>
                    {formData.net}
                  </SelectValue>
                        </SelectTrigger>
                      <SelectContent>
                        {Array.from({ length: 128 }, (_, i) => (
                    <SelectItem key={i} value={String(i)}>{i}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
            </div>
              
            <div>
              <Label className="text-sm font-medium text-foreground">Subnet</Label>
              <Select 
                value={formData.subnet}
                onValueChange={(value) => handleSelectChange("subnet", value)}
              >
                        <SelectTrigger>
                  <SelectValue>
                    {formData.subnet}
                  </SelectValue>
                        </SelectTrigger>
                      <SelectContent>
                        {Array.from({ length: 16 }, (_, i) => (
                    <SelectItem key={i} value={String(i)}>{i}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
            </div>
            </div>
            
            
          <div>
            <Label htmlFor="deviceName" className="text-sm font-medium text-foreground">Device Name</Label>
            <Input
              id="deviceName"
              name="deviceName"
              value={formData.deviceName}
              onChange={handleChange}
              placeholder="OpenLumen Node"
              maxLength={17}
              className={errors.deviceName ? "border-destructive focus-visible:ring-destructive" : ""}
            />
            <p className="mt-1 text-xs text-muted-foreground">Name will appear on network discovery (max 17 characters)</p>
            {errors.deviceName && (
              <p className="mt-1 text-sm text-destructive">{errors.deviceName}</p>
            )}
          </div>

          <div className="flex justify-end">
            <Button type="submit">Save ArtNet Settings</Button>
          </div>
          </form>
      </CardContent>
    </Card>
  );
}
