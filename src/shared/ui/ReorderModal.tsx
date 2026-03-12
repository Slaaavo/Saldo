import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  useSortable,
  verticalListSortingStrategy,
  arrayMove,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { GripVertical } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogBody,
  DialogFooter,
} from './dialog';
import { Button } from './button';

interface Item {
  id: number;
  name: string;
}

interface SortableRowProps {
  item: Item;
}

function SortableRow({ item }: SortableRowProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: item.id,
  });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-center gap-3 rounded-md border border-border bg-card px-3 py-2"
    >
      <button
        {...attributes}
        {...listeners}
        className="cursor-grab touch-none text-muted-foreground hover:text-foreground"
        aria-label="drag handle"
        tabIndex={-1}
      >
        <GripVertical className="h-4 w-4" />
      </button>
      <span className="text-sm">{item.name}</span>
    </div>
  );
}

interface Props {
  items: Item[];
  title: string;
  onSave: (orderedIds: number[]) => void;
  onClose: () => void;
}

export default function ReorderModal({ items, title, onSave, onClose }: Props) {
  const { t } = useTranslation();
  const [ordered, setOrdered] = useState<Item[]>(items);

  const sensors = useSensors(useSensor(PointerSensor));

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      setOrdered((prev) => {
        const oldIndex = prev.findIndex((item) => item.id === active.id);
        const newIndex = prev.findIndex((item) => item.id === over.id);
        return arrayMove(prev, oldIndex, newIndex);
      });
    }
  };

  return (
    <Dialog
      open={true}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-2">
          <p className="text-xs text-muted-foreground">{t('reorder.dragHint')}</p>

          <div className="flex flex-col gap-2 py-2">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              onDragEnd={handleDragEnd}
            >
              <SortableContext
                items={ordered.map((i) => i.id)}
                strategy={verticalListSortingStrategy}
              >
                {ordered.map((item) => (
                  <SortableRow key={item.id} item={item} />
                ))}
              </SortableContext>
            </DndContext>
          </div>
        </DialogBody>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            {t('reorder.cancel')}
          </Button>
          <Button type="button" onClick={() => onSave(ordered.map((i) => i.id))}>
            {t('reorder.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
