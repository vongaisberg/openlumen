import { useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { Card, CardContent } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Form, FormControl, FormField, FormItem, FormLabel, FormMessage } from "@/components/ui/form";

const artnetSchema = z.object({
  net: z.string(),
  subnet: z.string(),
  protocolVersion: z.enum(["ArtNet 3", "ArtNet 4"]),
  deviceName: z.string().max(17, {
    message: "Device name cannot exceed 17 characters",
  }),
});

type ArtnetFormValues = z.infer<typeof artnetSchema>;

interface ArtNetSettingsProps {
  data: any;
  onSave: (data: any) => void;
}

export default function ArtNetSettings({ data, onSave }: ArtNetSettingsProps) {
  const form = useForm<ArtnetFormValues>({
    resolver: zodResolver(artnetSchema),
    defaultValues: {
      net: "0",
      subnet: "0",
      protocolVersion: "ArtNet 3",
      deviceName: "ArtNet Node",
    },
  });

  useEffect(() => {
    if (data?.artnetConfig) {
      form.reset({
        net: data.artnetConfig.net.toString(),
        subnet: data.artnetConfig.subnet.toString(),
        protocolVersion: data.artnetConfig.protocolVersion,
        deviceName: data.artnetConfig.deviceName,
      });
    }
  }, [data, form]);

  function onSubmit(values: ArtnetFormValues) {
    onSave({
      ...values,
      net: parseInt(values.net),
      subnet: parseInt(values.subnet),
    });
  }

  return (
    <Card>
      <CardContent className="pt-6">
        <h2 className="text-lg font-medium text-gray-800 mb-6">ArtNet Configuration</h2>
        
        <Form {...form}>
          <form onSubmit={form.handleSubmit(onSubmit)} className="space-y-6">
            {/* Net and Subnet */}
            <div className="grid grid-cols-1 gap-6 sm:grid-cols-2">
              <FormField
                control={form.control}
                name="net"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel className="block text-sm font-medium text-gray-700">Net</FormLabel>
                    <Select onValueChange={field.onChange} defaultValue={field.value}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue placeholder="Select Net" />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {Array.from({ length: 128 }, (_, i) => (
                          <SelectItem key={i} value={i.toString()}>{i}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />
              
              <FormField
                control={form.control}
                name="subnet"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel className="block text-sm font-medium text-gray-700">Subnet</FormLabel>
                    <Select onValueChange={field.onChange} defaultValue={field.value}>
                      <FormControl>
                        <SelectTrigger>
                          <SelectValue placeholder="Select Subnet" />
                        </SelectTrigger>
                      </FormControl>
                      <SelectContent>
                        {Array.from({ length: 16 }, (_, i) => (
                          <SelectItem key={i} value={i.toString()}>{i}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <FormMessage />
                  </FormItem>
                )}
              />
            </div>
            
            {/* Protocol Version */}
            <FormField
              control={form.control}
              name="protocolVersion"
              render={({ field }) => (
                <FormItem className="space-y-1">
                  <FormLabel className="block text-sm font-medium text-gray-700">Protocol Version</FormLabel>
                  <FormControl>
                    <RadioGroup
                      onValueChange={field.onChange}
                      defaultValue={field.value}
                      className="flex items-center space-x-4 mt-2"
                    >
                      <div className="flex items-center">
                        <RadioGroupItem value="ArtNet 3" id="artnet3" />
                        <Label htmlFor="artnet3" className="ml-2 text-sm text-gray-700">ArtNet 3</Label>
                      </div>
                      <div className="flex items-center">
                        <RadioGroupItem value="ArtNet 4" id="artnet4" />
                        <Label htmlFor="artnet4" className="ml-2 text-sm text-gray-700">ArtNet 4</Label>
                      </div>
                    </RadioGroup>
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
            
            {/* Device Name */}
            <FormField
              control={form.control}
              name="deviceName"
              render={({ field }) => (
                <FormItem>
                  <FormLabel className="block text-sm font-medium text-gray-700">Device Name</FormLabel>
                  <FormControl>
                    <Input
                      placeholder="ArtNet Node"
                      maxLength={17}
                      {...field}
                    />
                  </FormControl>
                  <p className="mt-1 text-xs text-gray-500">Name will appear on network discovery (max 17 characters)</p>
                  <FormMessage />
                </FormItem>
              )}
            />
            
            {/* Submit Button */}
            <div className="flex justify-end">
              <Button type="submit" className="bg-blue-600 hover:bg-blue-700">
                Save ArtNet Settings
              </Button>
            </div>
          </form>
        </Form>
      </CardContent>
    </Card>
  );
}
