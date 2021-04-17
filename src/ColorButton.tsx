import {
  ColorPicker,
  CommandBarButton,
  IColorCellProps,
  Pivot,
  PivotItem,
  SwatchColorPicker,
} from "@fluentui/react";
import { SharedColors } from "@fluentui/theme";
import * as Color from "color";
import * as React from "react";

export interface ColorButtonProps {
  color: string;
  onColorChanged: (color: string) => void;
}

export const ColorButton = (props: ColorButtonProps): JSX.Element => {
  return (
    <CommandBarButton
      styles={{
        menuIcon: { display: "none" },
      }}
      title="Color"
      menuProps={{
        items: [{ key: "colors" }],
        onRenderMenuList: () => renderMenuList(),
      }}
    >
      <div
        style={{
          width: "16px",
          height: "16px",
          backgroundColor: props.color,
        }}
      />
    </CommandBarButton>
  );

  function renderMenuList(): JSX.Element {
    const color = new Color(props.color);
    const id = colorToId.get(color.hex());

    return (
      <Pivot>
        <PivotItem headerText="Swatch">
          <SwatchColorPicker
            cellShape={"square"}
            colorCells={colorCells}
            columnCount={10}
            onChange={(_, __, c) => {
              if (c !== undefined) {
                const newColor = new Color(c).alpha(color.alpha());
                props.onColorChanged(newColor.toString());
              }
            }}
            selectedId={id}
          />
        </PivotItem>
        <PivotItem headerText="Picker">
          <ColorPicker
            color={props.color}
            onChange={(_, c) => props.onColorChanged(c.str)}
          />
        </PivotItem>
      </Pivot>
    );
  }
};

const colorCells: IColorCellProps[] = [
  SharedColors.pinkRed10,
  SharedColors.red20,
  SharedColors.red10,
  SharedColors.redOrange20,
  SharedColors.redOrange10,
  SharedColors.orange30,
  SharedColors.orange20,
  SharedColors.orange10,
  SharedColors.yellow10,
  SharedColors.orangeYellow20,
  SharedColors.orangeYellow10,
  SharedColors.yellowGreen10,
  SharedColors.green20,
  SharedColors.green10,
  SharedColors.greenCyan10,
  SharedColors.cyan40,
  SharedColors.cyan30,
  SharedColors.cyan20,
  SharedColors.cyan10,
  SharedColors.cyanBlue20,
  SharedColors.cyanBlue10,
  SharedColors.blue10,
  SharedColors.blueMagenta40,
  SharedColors.blueMagenta30,
  SharedColors.blueMagenta20,
  SharedColors.blueMagenta10,
  SharedColors.magenta20,
  SharedColors.magenta10,
  SharedColors.magentaPink20,
  SharedColors.magentaPink10,
  SharedColors.gray40,
  SharedColors.gray30,
  SharedColors.gray20,
  SharedColors.gray10,
].map((c, i) => ({
  id: i.toString(),
  color: new Color(c).hex(),
}));

const colorToId: Map<string, string> = new Map(
  colorCells.map((c) => [c.color, c.id])
);
