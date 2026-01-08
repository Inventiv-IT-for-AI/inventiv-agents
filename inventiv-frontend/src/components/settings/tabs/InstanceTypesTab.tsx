"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Plus } from "lucide-react";
import { IADataTable, type IADataTableColumn, type DataTableSortState, type LoadRangeResult } from "ia-widgets";

export type InstanceTypesTabProps<Row> = {
  refreshTick: number;
  sort: DataTableSortState;
  setSort: (s: DataTableSortState) => void;
  columns: IADataTableColumn<Row>[];
  loadRange: (offset: number, limit: number) => Promise<LoadRangeResult<Row>>;
  onCreate: () => void;
};

export function InstanceTypesTab<Row>({
  refreshTick,
  sort,
  setSort,
  columns,
  loadRange,
  onCreate,
}: InstanceTypesTabProps<Row>) {
  return (
    <Card>
      <CardContent>
        <IADataTable<Row>
          listId="settings:types"
          title="Instance Types"
          dataKey={JSON.stringify({ refresh: refreshTick, sort })}
          rightHeader={
            <div className="flex gap-2">
              <Button size="sm" onClick={onCreate}>
                <Plus className="h-4 w-4 mr-2" />
                Ajouter
              </Button>
            </div>
          }
          autoHeight={true}
          height={300}
          rowHeight={52}
          columns={columns}
          loadRange={loadRange}
          sortState={sort}
          onSortChange={setSort}
          sortingMode="server"
        />
      </CardContent>
    </Card>
  );
}


