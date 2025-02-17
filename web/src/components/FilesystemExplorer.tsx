import { useEffect, useState } from "react";
import { useRecoilValue } from "recoil";
import { default as styled } from "styled-components";
import { clientState } from "../utils/atoms";
//@ts-ignore
import { FixedSizeList as List } from "react-window";
//@ts-ignore
import AutoSizer from "react-virtualized-auto-sizer";
import { ReactSVG } from "react-svg";
import { DirItemInfo } from "../types/client";

const ExplorerContainer = styled.div`
  margin: 5px;
  height: calc(100% - 10px);
`;

const ExplorerItemContainer = styled.div<{
  isFile: boolean;
  isOpened: boolean;
}>`
  max-width: 300px;
  display: flex;
  align-items: center;
  cursor: pointer;
  outline: 0;
  white-space: nowrap;
  position: relative;
  background: ${({ theme }) => theme.elements.explorer.item.background};
  color: ${({ theme }) => theme.elements.explorer.item.text.color};
  font-size: 12px;
  border-radius: 5px;
  line-break: none;
  text-overflow: elliptic;
  overflow: hidden;
  border: none;
  min-width: 170px;
  max-width: 200px;
  text-align: left;
  user-select: none;
  &:hover {
    background: ${({ theme }) => theme.elements.explorer.item.hover.background};
  }
  & .arrow svg {
    margin-right: 7px;
    margin-left: 15px;
    margin-top: 2px;
    width: 8px;
    transform: ${({ isOpened }) =>
      isOpened ? " rotate(0deg)" : " rotate(-90deg)"};
    & > rect {
      fill: ${({ theme }) => theme.elements.explorer.item.arrow.fill};
      stroke: ${({ theme }) => theme.elements.explorer.item.arrow.fill};
    }
  }
  & .file svg {
    width: 20px;
    margin-right: 4px;
    margin-top: 3px;
    ${({ isFile }) => isFile && "padding-left: 29px;"}
  }
`;

interface ExplorerOptions {
  // Route where the explorer opens in
  initialRoute: string;
  // Name of the FS installed in the Core
  filesystem_name: string;
  // Callback executed when a item is clicked
  onSelected: (path: TreeItemInfo) => void;
}

interface TreeItems {
  [key: string]: TreeItem;
}

export interface TreeItemInfo {
  name: string;
  isFile: boolean;
  path: string;
  depth: number;
}
interface TreeItem {
  items: TreeItems;
  name: string;
  isFile: boolean;
}

/*
 * Flat the folder tree into an array
 */
function mapTree(list: TreeItemInfo[], item: TreeItem, depth: number) {
  Object.keys(item.items).forEach((item_path) => {
    const itemInfo = item.items[item_path];
    list.push({
      path: item_path,
      name: itemInfo.name,
      isFile: itemInfo.isFile,
      depth,
    });
    mapTree(list, itemInfo, depth + 1);
  });
  return list;
}

/*
 * Remove a specific sub tree in the big tree
 */
function removeSubTreeByPath(tree: TreeItem, path: string): TreeItem {
  Object.keys(tree.items).forEach((item_path) => {
    if (path === item_path) {
      tree.items[item_path].items = {};
    }
    if (path.startsWith(item_path)) {
      removeSubTreeByPath(tree.items[item_path], path);
    }
  });
  return tree;
}

/*
 * Add items to a subtree
 */
function addItemsToSubTreeByPath(
  tree: TreeItem,
  path: string,
  subTreeItems: TreeItems
): TreeItem {
  Object.keys(tree.items).forEach((item_path) => {
    if (path === item_path) {
      tree.items[item_path].items = subTreeItems;
      return;
    }
    if (path.startsWith(item_path)) {
      addItemsToSubTreeByPath(tree.items[item_path], path, subTreeItems);
    }
  });
  return tree;
}

/*
 * Check if the subtree in the tree is opened or not
 */
function isSubTreeByPathOpened(tree: TreeItem, path: string): boolean {
  let res = false;
  Object.keys(tree.items).forEach((item_path) => {
    if (path === item_path) {
      if (Object.keys(tree.items[item_path].items).length > 0) {
        res = true;
        return;
      }
    }
    if (path.startsWith(item_path) && res === false) {
      res = isSubTreeByPathOpened(tree.items[item_path], path);
      return;
    }
  });
  return res;
}

/*
 * Convert a Items Info List into a TreeItems
 */
function mapItemsListToSubTreeItem(items: DirItemInfo[]): TreeItems {
  const subTreeItems: TreeItems = {};

  items.forEach(
    (item) =>
      (subTreeItems[item.path] = {
        name: item.name,
        isFile: item.is_file,
        items: {},
      })
  );

  return subTreeItems;
}

function FilesystemExplorer({
  initialRoute,
  onSelected,
  filesystem_name,
}: ExplorerOptions) {
  const client = useRecoilValue(clientState);
  const defaultState: [TreeItem, TreeItemInfo[]] = [
    {
      name: initialRoute,
      isFile: false,
      items: {},
    },
    [],
  ];
  const [[folderTree, folderItems], setFolderData] = useState(defaultState);

  useEffect(() => {
    // Load the given initial route
    client.list_dir_by_path(initialRoute, filesystem_name).then((pathItems) => {
      if (pathItems.Ok) {
        const subTree: TreeItem = {
          name: initialRoute,
          isFile: false,
          items: mapItemsListToSubTreeItem(pathItems.Ok),
        };
        setFolderData([subTree, mapTree([], subTree, 0)]);
      } else {
        // handle error
      }
    });
  }, [initialRoute]); // InitialRoute

  function closeFolder(path: string) {
    // Close the sub tree
    const newFolderTree = removeSubTreeByPath(folderTree, path);
    setFolderData([{ ...newFolderTree }, mapTree([], newFolderTree, 0)]);
  }

  function openFolder(path: string) {
    // Open the folder
    client.list_dir_by_path(path, filesystem_name).then((pathItems) => {
      if (pathItems.Ok) {
        const subTreeItems: TreeItems = mapItemsListToSubTreeItem(pathItems.Ok);

        // Add the new items to the sub tree
        addItemsToSubTreeByPath(folderTree, path, subTreeItems);

        setFolderData([{ ...folderTree }, mapTree([], folderTree, 0)]);
      } else {
        // handle error
      }
    });
  }

  function ListItem({ index, style }: { index: number; style: any }) {
    const itemInfo = folderItems[index];
    const itemStyle = {
      ...style,
      marginLeft: itemInfo.depth * 10,
    };
    const isOpened = isSubTreeByPathOpened(folderTree, itemInfo.path);

    // When the item is clicked
    function onClick() {
      // Trigger the select callback
      onSelected(itemInfo);

      // If folder
      if (!itemInfo.isFile) {
        if (isOpened) {
          // Close itself
          closeFolder(itemInfo.path);
        } else {
          // Open itself
          openFolder(itemInfo.path);
        }
      }
    }

    return (
      <ExplorerItemContainer
        key={itemInfo.path}
        onClick={onClick}
        style={itemStyle}
        isFile={itemInfo.isFile}
        isOpened={isOpened}
        title={itemInfo.path}
      >
        {!itemInfo.isFile && (
          <ReactSVG src="/icons/collapse_arrow.svg" className="arrow" />
        )}
        {itemInfo.isFile ? (
          <ReactSVG src="/icons/files/unknown.svg" className="file" />
        ) : isOpened ? (
          <ReactSVG src="/icons/files/folder_opened.svg" className="file" />
        ) : (
          <ReactSVG src="/icons/files/folder_closed.svg" className="file" />
        )}
        <span>{itemInfo.name}</span>
      </ExplorerItemContainer>
    );
  }

  return (
    <ExplorerContainer>
      <AutoSizer>
        {({ height, width }: { height: number; width: number }) => {
          return (
            <List
              itemCount={folderItems.length}
              width={width}
              height={height}
              itemSize={26}
              overscanCount={10}
            >
              {ListItem}
            </List>
          );
        }}
      </AutoSizer>
    </ExplorerContainer>
  );
}

export default FilesystemExplorer;
