"use client";

import { useEffect, useState } from "react";
import { formatDistanceToNow, parseISO } from "date-fns";
import { Badge } from "@/components/ui/badge";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Table, TableHeader, TableRow, TableHead, TableBody, TableCell } from "@/components/ui/table";
import { Plus, RefreshCcw, Server, Activity, AlertCircle, CheckCircle, Copy, Check, Eye, Archive } from "lucide-react";

type Instance = {
  id: string;
  provider_id: string;
  provider_name: string; // NEW
  zone: string;
  region: string; // NEW
  instance_type: string;
  status: string;
  ip_address: string | null;
  created_at: string;
  gpu_vram?: number; // NEW
  cost_per_hour?: number; // NEW
  total_cost?: number;    // NEW
};

const CopyButton = ({ text }: { text: string }) => {
  const [copied, setCopied] = useState(false);

  const onCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Button variant="ghost" size="icon" className="h-6 w-6 ml-2" onClick={onCopy}>
      {copied ? <Check className="h-3 w-3 text-green-500" /> : <Copy className="h-3 w-3 text-muted-foreground" />}
    </Button>
  );
};

type Provider = { id: string; name: string };
type Region = { id: string; name: string; code: string; is_active: boolean };
type Zone = { id: string; name: string; code: string; is_active: boolean };
type InstanceType = { id: string; name: string; code: string; cost_per_hour: number | null; is_active: boolean };

export default function Dashboard() {
  const [instances, setInstances] = useState<Instance[]>([]);

  // Deploy Modal State
  const [isDeployOpen, setIsDeployOpen] = useState(false);
  const [deployStep, setDeployStep] = useState<'form' | 'submitting' | 'success'>('form');

  // Catalog Data
  const [providers, setProviders] = useState<Provider[]>([]);
  const [regions, setRegions] = useState<Region[]>([]);
  const [zones, setZones] = useState<Zone[]>([]);
  const [allZones, setAllZones] = useState<Zone[]>([]); // Keep all zones for filtering
  const [instanceTypes, setInstanceTypes] = useState<InstanceType[]>([]);

  // Selected Values
  const [selectedProviderId, setSelectedProviderId] = useState<string>("");
  const [selectedRegionId, setSelectedRegionId] = useState<string>("");
  const [selectedZoneId, setSelectedZoneId] = useState<string>("");
  const [selectedTypeId, setSelectedTypeId] = useState<string>("");

  // Derived data
  const selectedType = instanceTypes.find(t => t.id === selectedTypeId);

  // Terminate Modal State
  const [isTerminateOpen, setIsTerminateOpen] = useState(false);
  const [terminateStep, setTerminateStep] = useState<'confirm' | 'submitting' | 'success'>('confirm');
  const [instanceToTerminate, setInstanceToTerminate] = useState<string | null>(null);

  const openTerminateModal = (id: string) => {
    setInstanceToTerminate(id);
    setTerminateStep('confirm');
    setIsTerminateOpen(true);
  };

  // Details Modal State
  const [isDetailsOpen, setIsDetailsOpen] = useState(false);
  const [selectedInstance, setSelectedInstance] = useState<Instance | null>(null);

  const openDetailsModal = (instance: Instance) => {
    setSelectedInstance(instance);
    setIsDetailsOpen(true);
  };

  const handleArchive = async (id: string) => {
    try {
      const res = await fetch(apiUrl(`instances/${id}/archive`), { method: "PUT" });
      if (res.ok) {
        setInstances(prev => prev.filter(i => i.id !== id));
      } else {
        alert("Failed to archive");
      }
    } catch (e) {
      console.error(e);
      alert("Error archiving instance");
    }
  };

  // Fetch Catalog Data when modal opens
  useEffect(() => {
    if (!isDeployOpen) return;

    const fetchCatalog = async () => {
      try {
        // Fetch all catalog data
        const [regionsRes, zonesRes, typesRes] = await Promise.all([
          fetch(apiUrl("regions")),
          fetch(apiUrl("zones")),
          fetch(apiUrl("instance_types"))
        ]);

        if (regionsRes.ok) {
          const data: Region[] = await regionsRes.json();
          setRegions(data.filter(r => r.is_active));
        }
        if (zonesRes.ok) {
          const data: Zone[] = await zonesRes.json();
          const activeZones = data.filter(z => z.is_active);
          setAllZones(activeZones); // Store all zones
          setZones(activeZones); // Display all zones initially
        }
        if (typesRes.ok) {
          const data: InstanceType[] = await typesRes.json();
          setInstanceTypes(data.filter(t => t.is_active));
        }

        // Hardcoded provider for now (Scaleway)
        setProviders([{ id: "00000000-0000-0000-0000-000000000001", name: "Scaleway" }]);
        setSelectedProviderId("00000000-0000-0000-0000-000000000001");
      } catch (err) {
        console.error("Failed to fetch catalog", err);
      }
    };

    fetchCatalog();
  }, [isDeployOpen]);

  // Filter zones by selected region
  useEffect(() => {
    if (!selectedRegionId) {
      setZones(allZones);
      return;
    }

    const region = regions.find(r => r.id === selectedRegionId);
    if (region) {
      // Filter zones that belong to the selected region
      const filteredZones = allZones.filter(z => z.code.startsWith(region.code));
      setZones(filteredZones);

      // Reset zone and type selections when region changes
      setSelectedZoneId("");
      setSelectedTypeId("");
    }
  }, [selectedRegionId, regions, allZones]);

  // Filter instance types by selected zone
  useEffect(() => {
    if (!selectedZoneId) {
      // If no zone selected, fetch all instance types
      const fetchAllTypes = async () => {
        try {
          const res = await fetch(apiUrl("instance_types"));
          if (res.ok) {
            const data: InstanceType[] = await res.json();
            setInstanceTypes(data.filter(t => t.is_active));
          }
        } catch (err) {
          console.error("Failed to fetch instance types", err);
        }
      };
      fetchAllTypes();
      return;
    }

    // Fetch instance types available for the selected zone
    const fetchTypesForZone = async () => {
      try {
        const res = await fetch(apiUrl(`zones/${selectedZoneId}/instance_types`));
        if (res.ok) {
          const data: InstanceType[] = await res.json();
          setInstanceTypes(data);
          // Reset instance type selection when zone changes
          setSelectedTypeId("");
        } else {
          // Fallback to all types if endpoint fails
          console.warn("Failed to fetch zone-specific types, showing all");
        }
      } catch (err) {
        console.error("Failed to fetch instance types for zone", err);
      }
    };
    fetchTypesForZone();
  }, [selectedZoneId]);

  const handleDeploy = async () => {
    if (!selectedZoneId || !selectedTypeId) {
      alert("Please select all required fields");
      return;
    }

    setDeployStep('submitting');
    try {
      const selectedZone = zones.find(z => z.id === selectedZoneId);
      const selectedType = instanceTypes.find(t => t.id === selectedTypeId);

      const res = await fetch(apiUrl("deployments"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          zone: selectedZone?.code || "",
          instance_type: selectedType?.code || ""
        })
      });

      if (res.ok) {
        setDeployStep('success');
        setTimeout(() => {
          setIsDeployOpen(false);
          setDeployStep('form');
          // Reset form
          setSelectedRegionId("");
          setSelectedZoneId("");
          setSelectedTypeId("");
          // Trigger a refresh
          window.dispatchEvent(new Event('refresh-instances'));
        }, 2000);
      } else {
        alert("Deployment failed!");
        setIsDeployOpen(false);
        setDeployStep('form');
      }
    } catch (e) {
      console.error(e);
      alert("Error deploying instance.");
      setIsDeployOpen(false);
      setDeployStep('form');
    }
  };

  // Fetch Instances (Backend Only)
  useEffect(() => {
    const fetchData = async () => {
      try {
        const res = await fetch(apiUrl("instances"));
        if (res.ok) {
          const data = await res.json();
          setInstances(data);
        }
      } catch (err) {
        console.error("Polling Error:", err);
      }
    };

    fetchData(); // Initial
    const interval = setInterval(fetchData, 5000); // Poll every 5s

    // Listen for manual refreshes
    window.addEventListener('refresh-instances', fetchData);

    return () => {
      clearInterval(interval);
      window.removeEventListener('refresh-instances', fetchData);
    };
  }, []);

  const handleConfirmTerminate = async () => {
    if (!instanceToTerminate) return;
    setTerminateStep('submitting');

    try {
      const res = await fetch(apiUrl(`instances/${instanceToTerminate}`), { method: "DELETE" });
      if (res.ok) {
        setTerminateStep('success');
        // Auto close after 1.5s
        setTimeout(() => {
          setIsTerminateOpen(false);
          setInstanceToTerminate(null);
          // Optimistic update
          setInstances(prev => prev.filter(i => i.id !== instanceToTerminate));
        }, 1500);
      } else {
        alert("Failed to terminate.");
        setIsTerminateOpen(false);
      }
    } catch (e) {
      console.error(e);
      alert("Error terminating instance.");
      setIsTerminateOpen(false);
    }
  };

  return (
    <div className="p-8 space-y-8">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
          <p className="text-muted-foreground">Overview of your GPU infrastructure.</p>
        </div>
        <div className="flex space-x-2">
          <Button variant="outline" size="icon" onClick={() => window.location.reload()}>
            <RefreshCcw className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <Dialog open={isTerminateOpen} onOpenChange={setIsTerminateOpen}>
          <DialogContent className="sm:max-w-[425px]">
            <DialogHeader>
              <DialogTitle>Terminate Instance</DialogTitle>
              <DialogDescription>
                Are you sure you want to shut down this instance? This action cannot be undone.
              </DialogDescription>
            </DialogHeader>

            {terminateStep === 'success' ? (
              <div className="flex flex-col items-center justify-center py-6 space-y-4 text-red-600 animate-in fade-in zoom-in duration-300">
                <CheckCircle className="h-16 w-16" />
                <span className="text-xl font-bold">Instance Terminée</span>
              </div>
            ) : (
              <div className="py-4 text-sm text-muted-foreground">
                Instance ID: <span className="font-mono text-foreground">{instanceToTerminate}</span>
              </div>
            )}

            <DialogFooter>
              {terminateStep !== 'success' && (
                <>
                  <Button variant="outline" onClick={() => setIsTerminateOpen(false)} disabled={terminateStep === 'submitting'}>
                    Cancel
                  </Button>
                  <Button variant="destructive" onClick={handleConfirmTerminate} disabled={terminateStep === 'submitting'}>
                    {terminateStep === 'submitting' ? "Terminating..." : "Confirm Terminate"}
                  </Button>
                </>
              )}
            </DialogFooter>
          </DialogContent>
        </Dialog>

        <Dialog open={isDetailsOpen} onOpenChange={setIsDetailsOpen}>
          <DialogContent className="sm:max-w-[600px]">
            <DialogHeader>
              <DialogTitle>Instance Details</DialogTitle>
            </DialogHeader>
            {selectedInstance && (
              <div className="grid gap-6 py-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Identity</h4>
                    <div className="space-y-1 text-sm">
                      <div className="flex justify-between border-b pb-1"><span>ID</span> <span className="font-mono text-xs">{selectedInstance.id.split('-')[0]}...</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Provider</span> <span className="font-medium">{selectedInstance.provider_name}</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Region</span> <span>{selectedInstance.region}</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Zone</span> <span>{selectedInstance.zone}</span></div>
                    </div>
                  </div>
                  <div>
                    <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Specs & Status</h4>
                    <div className="space-y-1 text-sm">
                      <div className="flex justify-between border-b pb-1"><span>Type</span> <span className="font-medium">{selectedInstance.instance_type}</span></div>
                      <div className="flex justify-between border-b pb-1"><span>VRAM</span> <span>{selectedInstance.gpu_vram ? `${selectedInstance.gpu_vram} GB` : '-'}</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Rate</span> <span>${selectedInstance.cost_per_hour}/hr</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Total Cost</span> <span className="font-bold text-green-600">${selectedInstance.total_cost?.toFixed(4)}</span></div>
                      <div className="flex justify-between border-b pb-1"><span>Status</span> <Badge variant="outline">{selectedInstance.status}</Badge></div>
                      <div className="flex justify-between border-b pb-1"><span>Created</span> <span>{formatDistanceToNow(parseISO(selectedInstance.created_at), { addSuffix: true })}</span></div>
                    </div>
                  </div>
                </div>
                <div>
                  <h4 className="font-semibold mb-2 text-sm text-muted-foreground">Network</h4>
                  <div className="p-3 bg-muted rounded-md font-mono text-sm flex justify-between items-center">
                    <span>{selectedInstance.ip_address || "No Public IP"}</span>
                    {selectedInstance.ip_address && <CopyButton text={selectedInstance.ip_address} />}
                  </div>
                </div>
              </div>
            )}
          </DialogContent>
        </Dialog>
      </div >

      {/* Stats */}
      < div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4" >
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Total Instances</CardTitle>
            <Server className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{instances.length}</div>
            <p className="text-xs text-muted-foreground">All time managed</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Active</CardTitle>
            <Activity className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-green-600">{instances.filter(i => i.status.toLowerCase() === 'ready').length}</div>
            <p className="text-xs text-muted-foreground">Operational</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Provisioning</CardTitle>
            <RefreshCcw className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-blue-600">{instances.filter(i => ['provisioning', 'booting'].includes(i.status.toLowerCase())).length}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
            <CardTitle className="text-sm font-medium">Failed/Terminated</CardTitle>
            <AlertCircle className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-muted-foreground">{instances.filter(i => ['failed', 'terminated'].includes(i.status.toLowerCase())).length}</div>
          </CardContent>
        </Card>
      </div >

      {/* Instances Table */}
      < Card className="col-span-4" >
        <CardHeader className="flex flex-row items-center justify-between space-y-0">
          <CardTitle>Instances</CardTitle>
          <Dialog open={isDeployOpen} onOpenChange={setIsDeployOpen}>
            <DialogTrigger asChild>
              <Button>
                <Plus className="mr-2 h-4 w-4" /> Create Instance
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-[500px]">
              <DialogHeader>
                <DialogTitle>Create New Instance</DialogTitle>
                <DialogDescription>
                  Configure your GPU instance parameters.
                </DialogDescription>
              </DialogHeader>

              {deployStep === 'success' ? (
                <div className="flex flex-col items-center justify-center py-6 space-y-4 text-green-600 animate-in fade-in zoom-in duration-300">
                  <CheckCircle className="h-16 w-16" />
                  <span className="text-xl font-bold">Instance Created!</span>
                </div>
              ) : (
                <div className="grid gap-4 py-4">
                  <div className="grid grid-cols-4 items-center gap-4">
                    <Label className="text-right">Provider</Label>
                    <Select value={selectedProviderId} onValueChange={setSelectedProviderId} disabled>
                      <SelectTrigger className="col-span-3">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {providers.map(p => (
                          <SelectItem key={p.id} value={p.id}>{p.name}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid grid-cols-4 items-center gap-4">
                    <Label className="text-right">Region</Label>
                    <Select value={selectedRegionId} onValueChange={(val) => {
                      setSelectedRegionId(val);
                      setSelectedZoneId(""); // Reset zone when region changes
                    }}>
                      <SelectTrigger className="col-span-3">
                        <SelectValue placeholder="Select region" />
                      </SelectTrigger>
                      <SelectContent>
                        {regions.map(r => (
                          <SelectItem key={r.id} value={r.id}>{r.name}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid grid-cols-4 items-center gap-4">
                    <Label className="text-right">Zone</Label>
                    <Select value={selectedZoneId} onValueChange={setSelectedZoneId} disabled={!selectedRegionId}>
                      <SelectTrigger className="col-span-3">
                        <SelectValue placeholder="Select zone" />
                      </SelectTrigger>
                      <SelectContent>
                        {zones.map(z => (
                          <SelectItem key={z.id} value={z.id}>{z.name}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  <div className="grid grid-cols-4 items-center gap-4">
                    <Label className="text-right">Instance Type</Label>
                    <Select value={selectedTypeId} onValueChange={setSelectedTypeId}>
                      <SelectTrigger className="col-span-3">
                        <SelectValue placeholder="Select type" />
                      </SelectTrigger>
                      <SelectContent>
                        {instanceTypes.map(t => (
                          <SelectItem key={t.id} value={t.id}>
                            {t.name} {t.cost_per_hour && `(${t.cost_per_hour}$/h)`}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>

                  {selectedType && selectedType.cost_per_hour && (
                    <div className="grid grid-cols-4 items-center gap-4 bg-muted/50 p-3 rounded-md">
                      <Label className="text-right text-muted-foreground">Cost</Label>
                      <p className="col-span-3 font-semibold">${selectedType.cost_per_hour}/hour</p>
                    </div>
                  )}
                </div>
              )}

              <DialogFooter>
                {deployStep !== 'success' && (
                  <Button type="submit" onClick={handleDeploy} disabled={deployStep === 'submitting'}>
                    {deployStep === 'submitting' ? "Deploying..." : "Create Instance"}
                  </Button>
                )}
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </CardHeader>
        <CardContent>
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>ID</TableHead>
                <TableHead>Provider</TableHead>
                <TableHead>Region</TableHead>
                <TableHead>Zone</TableHead>
                <TableHead>Type</TableHead>
                <TableHead>Cost</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Created</TableHead>
                <TableHead>IP Address</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {instances.map((instance) => (
                <TableRow key={instance.id}>
                  <TableCell className="font-mono text-xs">{instance.id.split('-')[0]}...</TableCell>
                  <TableCell>{instance.provider_name}</TableCell>
                  <TableCell>{instance.region}</TableCell>
                  <TableCell>{instance.zone}</TableCell>
                  <TableCell>{instance.instance_type}</TableCell>
                  <TableCell className="font-mono">
                    {instance.total_cost !== undefined ? `$${instance.total_cost.toFixed(4)}` : '-'}
                  </TableCell>
                  <TableCell>
                    <Badge variant={instance.status.toLowerCase() === 'ready' ? 'default' : instance.status.toLowerCase() === 'terminated' ? 'destructive' : 'secondary'}>
                      {instance.status}
                    </Badge>
                  </TableCell>
                  <TableCell className="whitespace-nowrap text-muted-foreground">
                  </TableCell>
                  <TableCell className="font-mono text-sm">
                    {instance.ip_address ? (
                      <div className="flex items-center">
                        {instance.ip_address}
                        <CopyButton text={instance.ip_address} />
                      </div>
                    ) : (
                      <span className="text-muted-foreground">-</span>
                    )}
                  </TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end space-x-2">
                      <Button variant="ghost" size="icon" onClick={() => openDetailsModal(instance)}>
                        <Eye className="h-4 w-4" />
                      </Button>
                      {instance.status.toLowerCase() !== 'terminated' && (
                        <Button variant="destructive" size="sm" onClick={() => openTerminateModal(instance.id)}>
                          Terminate
                        </Button>
                      )}
                      {instance.status.toLowerCase() === 'terminated' && (
                        <Button variant="secondary" size="icon" onClick={() => handleArchive(instance.id)} title="Archive">
                          <Archive className="h-4 w-4" />
                        </Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
              {instances.length === 0 && (
                <TableRow>
                  <TableCell colSpan={7} className="text-center h-24 text-muted-foreground">
                    No instances found.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </CardContent>
      </Card >
    </div >
  );
}
