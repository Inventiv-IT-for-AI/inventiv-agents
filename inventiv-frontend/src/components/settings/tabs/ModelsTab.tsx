"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Plus } from "lucide-react";
import { IADataTable, type IADataTableColumn, type DataTableSortState } from "ia-widgets";

export type ModelsTabProps<Row> = {
  refreshTick: number;
  sort: DataTableSortState;
  setSort: (s: DataTableSortState) => void;
  columns: IADataTableColumn<Row>[];
  rows: Row[];
  loading?: boolean;
  onCreate: () => void;
};

export function ModelsTab<Row>({ refreshTick, sort, setSort, columns, rows, loading, onCreate }: ModelsTabProps<Row>) {
  return (
    <Card>
      <CardContent>
        <IADataTable<Row>
          listId="settings:models"
          title="Models"
          dataKey={JSON.stringify({ refresh: refreshTick, sort })}
          rightHeader={
            <div className="flex gap-2">
              <Button size="sm" onClick={onCreate} disabled={!!loading}>
                <Plus className="h-4 w-4 mr-2" />
                Ajouter
              </Button>
            </div>
          }
          autoHeight={true}
          height={300}
          rowHeight={52}
          columns={columns}
          rows={rows}
          sortState={sort}
          onSortChange={setSort}
          sortingMode="server"
        />
      </CardContent>
    </Card>
  );
}


