import {
  CommandBarButton,
  Icon,
  Spinner,
  SpinnerSize,
  useTheme,
} from "@fluentui/react";
import * as React from "react";
import { useRef } from "react";
import { DraggableProvidedDragHandleProps } from "react-beautiful-dnd";
import { useDispatch } from "react-redux";
import { Bar } from "./Bar";
import { ColorButton } from "./ColorButton";
import { removeGraph, useSelector } from "./models/app";
import { setGraphColor, setGraphRelation } from "./models/graph";
import { RelationInput, RelationInputActions } from "./RelationInput";
import { SymbolsButton } from "./SymbolsButton";

export interface GraphBarProps {
  dragHandleProps?: DraggableProvidedDragHandleProps;
  focusGraphView: () => void;
  graphId: string;
}

export const GraphBar = (props: GraphBarProps): JSX.Element => {
  const dispatch = useDispatch();
  const graph = useSelector((s) => s.graphs.byId[props.graphId]);
  const theme = useTheme();
  const relationInputActionsRef = useRef<RelationInputActions>(null);

  return (
    <Bar>
      <div
        style={{
          color: theme.semanticColors.disabledBodyText,
          display: "grid",
          minWidth: "32px",
        }}
        {...props.dragHandleProps}
      >
        <Icon
          iconName="GripperDotsVertical"
          styles={{ root: { margin: "auto" } }}
        />
      </div>
      <ColorButton
        color={graph.color}
        onColorChanged={(c) => dispatch(setGraphColor(props.graphId, c))}
      />
      <RelationInput
        actionsRef={relationInputActionsRef}
        grow
        onEnterKeyPressed={props.focusGraphView}
        onRelationChanged={(r) => dispatch(setGraphRelation(props.graphId, r))}
        relation={graph.relation}
      />
      {graph.isProcessing && <Spinner size={SpinnerSize.small} />}
      <SymbolsButton
        onSymbolChosen={(symbol: string) =>
          relationInputActionsRef.current?.insertSymbol(symbol)
        }
        onSymbolPairChosen={(first: string, second: string) =>
          relationInputActionsRef.current?.insertSymbolPair(first, second)
        }
      />
      <CommandBarButton
        iconProps={{ iconName: "More" }}
        menuProps={{
          items: [
            {
              key: "remove",
              text: "Remove",
              iconProps: { iconName: "Delete" },
              onClick: () => {
                dispatch(removeGraph(props.graphId));
              },
            },
          ],
        }}
        styles={{
          menuIcon: { display: "none" },
        }}
        title="Actions"
      />
    </Bar>
  );
};
