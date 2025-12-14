import type { LucideIcon } from "lucide-react";
import { ChevronDown, ChevronRight, Server, Zap, Cloud, Database, Archive, AlertTriangle, Clock, CheckCircle } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { useState, useEffect } from "react";
import { apiUrl } from "@/lib/api";
import type { ActionLog } from "@/lib/types";

interface InstanceTimelineModalProps {
  open: boolean;
  onClose: () => void;
  instanceId: string;
}

export function InstanceTimelineModal({
  open,
  onClose,
  instanceId,
}: InstanceTimelineModalProps) {
  const [logs, setLogs] = useState<ActionLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedMetadata, setExpandedMetadata] = useState<Set<string>>(
    new Set()
  );

  useEffect(() => {
    if (open && instanceId) {
      fetchInstanceLogs();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, instanceId]);

  const fetchInstanceLogs = async () => {
    setLoading(true);
    try {
      const response = await fetch(
        apiUrl(`action_logs?instance_id=${instanceId}&limit=100`)
      );
      const data: ActionLog[] = await response.json();
      // Sort by created_at to show chronological order
      const sorted = data.sort(
        (a, b) =>
          new Date(a.created_at).getTime() - new Date(b.created_at).getTime()
      );
      setLogs(sorted);
    } catch (error) {
      console.error("Failed to fetch instance logs:", error);
    } finally {
      setLoading(false);
    }
  };

  const toggleMetadata = (logId: string) => {
    const newSet = new Set(expandedMetadata);
    if (newSet.has(logId)) newSet.delete(logId);
    else newSet.add(logId);
    setExpandedMetadata(newSet);
  };

  const getActionIcon = (actionType: string) => {
    const iconMap: Record<string, LucideIcon> = {
      REQUEST_CREATE: Zap,
      EXECUTE_CREATE: Server,
      PROVIDER_CREATE: Cloud,
      PROVIDER_START: Cloud,
      PROVIDER_GET_IP: Cloud,
      INSTANCE_CREATED: Database,
      INSTANCE_READY: CheckCircle,
      REQUEST_TERMINATE: Zap,
      EXECUTE_TERMINATE: Server,
      PROVIDER_TERMINATE: Cloud,
      TERMINATION_PENDING: Clock,
      TERMINATOR_RETRY: Cloud,
      TERMINATION_CONFIRMED: Database,
      INSTANCE_TERMINATED: Database,
      ARCHIVE_INSTANCE: Archive,
      PROVIDER_DELETED_DETECTED: AlertTriangle,
      HEALTH_CHECK: Clock,
    };
    return iconMap[actionType] || Server;
  };

  const getActionColor = (actionType: string) => {
    const colorMap: Record<string, string> = {
      REQUEST_CREATE: "border-blue-500 bg-blue-50",
      EXECUTE_CREATE: "border-purple-500 bg-purple-50",
      PROVIDER_CREATE: "border-orange-500 bg-orange-50",
      PROVIDER_START: "border-orange-500 bg-orange-50",
      PROVIDER_GET_IP: "border-orange-500 bg-orange-50",
      INSTANCE_CREATED: "border-green-500 bg-green-50",
      INSTANCE_READY: "border-green-600 bg-green-50",
      REQUEST_TERMINATE: "border-blue-600 bg-blue-50",
      EXECUTE_TERMINATE: "border-purple-600 bg-purple-50",
      PROVIDER_TERMINATE: "border-orange-600 bg-orange-50",
      TERMINATION_PENDING: "border-yellow-600 bg-yellow-50",
      TERMINATOR_RETRY: "border-orange-600 bg-orange-50",
      TERMINATION_CONFIRMED: "border-red-500 bg-red-50",
      INSTANCE_TERMINATED: "border-red-500 bg-red-50",
      ARCHIVE_INSTANCE: "border-gray-600 bg-gray-50",
      PROVIDER_DELETED_DETECTED: "border-yellow-600 bg-yellow-50",
      HEALTH_CHECK: "border-teal-600 bg-teal-50",
    };
    return colorMap[actionType] || "border-gray-500 bg-gray-50";
  };

  const getStatusIcon = (status: string) => {
    if (status === "success")
      return <span className="text-green-500 text-2xl">✅</span>;
    if (status === "failed")
      return <span className="text-red-500 text-2xl">❌</span>;
    return <span className="text-yellow-500 text-2xl">⏳</span>;
  };

  const formatDuration = (ms: number | null) => {
    if (!ms) return "-";
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
    return `${(ms / 60000).toFixed(2)}min`;
  };

  const formatTime = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  };

  return (
    <Dialog open={open} onOpenChange={onClose}>
      <DialogContent className="max-w-4xl max-h-[80vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center justify-between">
            <div>
              <h2 className="text-2xl font-bold">Instance Timeline</h2>
              <p className="text-sm text-muted-foreground font-mono mt-1">
                {instanceId}
              </p>
            </div>
          </DialogTitle>
        </DialogHeader>

        {loading ? (
          <div className="flex justify-center items-center py-12">
            <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary" />
          </div>
        ) : logs.length === 0 ? (
          <div className="text-center py-12 text-muted-foreground">
            No logs found for this instance
          </div>
        ) : (
          <div className="space-y-4 py-4">
            {logs.map((log, index) => {
              const Icon = getActionIcon(log.action_type);
              const isExpanded = expandedMetadata.has(log.id);
              const hasMetadata =
                log.metadata && Object.keys(log.metadata).length > 0;

              return (
                <div key={log.id} className="relative">
                  {/* Timeline connector */}
                  {index < logs.length - 1 && (
                    <div className="absolute left-6 top-16 bottom-0 w-0.5 bg-gradient-to-b from-gray-300 to-transparent h-8" />
                  )}

                  <Card
                    className={`border-l-4 ${getActionColor(
                      log.action_type
                    )} p-4 shadow-sm hover:shadow-md transition-shadow`}
                  >
                    <div className="flex items-start gap-4">
                      {/* Icon */}
                      <div className="flex-shrink-0">
                        <div
                          className={`p-3 rounded-full ${getActionColor(
                            log.action_type
                          )} border-2`}
                        >
                          <Icon className="h-5 w-5" />
                        </div>
                      </div>

                      {/* Content */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center justify-between mb-2">
                          <div className="flex items-center gap-3">
                            {getStatusIcon(log.status)}
                            <div>
                              <h3 className="font-bold text-lg">
                                {log.action_type
                                  .replace(/_/g, " ")
                                  .replace(/\b\w/g, (l) => l.toUpperCase())}
                              </h3>
                              <p className="text-sm text-muted-foreground">
                                {formatTime(log.created_at)} • Component:{" "}
                                <span className="font-semibold">
                                  {log.component}
                                </span>
                              </p>
                            </div>
                          </div>
                          <div className="text-right">
                            <Badge variant="outline" className="font-mono">
                              {formatDuration(log.duration_ms)}
                            </Badge>
                          </div>
                        </div>

                        {/* Error Message */}
                        {log.error_message && (
                          <div className="mt-2 p-2 bg-red-50 border border-red-200 rounded text-sm text-red-700">
                            <strong>Error:</strong> {log.error_message}
                          </div>
                        )}

                        {/* Metadata */}
                        {hasMetadata && (
                          <div className="mt-3">
                            <button
                              onClick={() => toggleMetadata(log.id)}
                              className="flex items-center gap-2 text-sm font-medium text-blue-600 hover:text-blue-800"
                            >
                              {isExpanded ? (
                                <ChevronDown className="h-4 w-4" />
                              ) : (
                                <ChevronRight className="h-4 w-4" />
                              )}
                              <span>
                                Metadata ({Object.keys(log.metadata!).length}{" "}
                                fields)
                              </span>
                            </button>
                            {isExpanded && (
                              <div className="mt-2 p-3 bg-gray-50 rounded font-mono text-xs border">
                                <pre className="whitespace-pre-wrap">
                                  {JSON.stringify(log.metadata, null, 2)}
                                </pre>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  </Card>
                </div>
              );
            })}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}


