import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { LucideIcon } from "lucide-react";

type StatsCardProps = {
    title: string;
    value: string | number;
    description?: string;
    icon: LucideIcon;
    iconClassName?: string;
    valueClassName?: string;
};

export function StatsCard({
    title,
    value,
    description,
    icon: Icon,
    iconClassName = "text-muted-foreground",
    valueClassName = "",
}: StatsCardProps) {
    return (
        <Card>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                <CardTitle className="text-sm font-medium">{title}</CardTitle>
                <Icon className={`h-4 w-4 ${iconClassName}`} />
            </CardHeader>
            <CardContent>
                <div className={`text-2xl font-bold ${valueClassName}`}>{value}</div>
                {description && (
                    <p className="text-xs text-muted-foreground">{description}</p>
                )}
            </CardContent>
        </Card>
    );
}
