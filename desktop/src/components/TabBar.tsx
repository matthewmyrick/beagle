// The section tab bar for the selected incident.

import type { JSX } from "react";

import { TABS } from "../lib/sections";

interface TabBarProps {
  activeFile: string;
  onSelect: (file: string) => void;
}

export function TabBar({ activeFile, onSelect }: TabBarProps): JSX.Element {
  return (
    <div className="tab-bar" role="tablist">
      {TABS.map((section, index) => (
        <button
          key={section.file}
          type="button"
          role="tab"
          aria-selected={section.file === activeFile}
          className={section.file === activeFile ? "tab active" : "tab"}
          onClick={() => {
            onSelect(section.file);
          }}
        >
          {index + 1} {section.title}
        </button>
      ))}
    </div>
  );
}
