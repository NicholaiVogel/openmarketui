import { useKeyboard } from "@opentui/react";
import { useGardenStore } from "./useGardenStore";
import { useThemeStore } from "./useThemeStore";
import { useModeStore, EDIT_FIELDS } from "./useModeStore";
import { keybindings, matchesKey } from "../config/keybindings";
import type { TradingMode } from "../types/mode";

interface UseKeyboardNavOptions {
  onQuit?: () => void;
  onReconnect?: () => void;
}

export function useKeyboardNav(options: UseKeyboardNavOptions = {}) {
  const {
    activeTab,
    setActiveTab,
    selectedIndex,
    moveSelection,
    toggleHelp,
    acknowledgeAllAlerts,
    pauseEngine,
    resumeEngine,
    toggleSpecimen,
    adjustSpecimenWeight,
    togglePositionsViewMode,
    beds,
    positions,
    engineStatus,
    setSelectedMarket,
    setSelectedSpecimen,
  } = useGardenStore();

  const { toggleMenu, handleMenuKey, menuOpen } = useThemeStore();
  const {
    menuScreen,
    menuIndex,
    viewMode,
    sessionStatus,
    configPresets,
    selectedPresetIndex,
    editFieldIndex,
    dateRangeIndex,
    dataAvailability,
    customStartDate,
    customEndDate,
    openModeMenu,
    closeModeMenu,
    setMenuScreen,
    moveMenuIndex: moveModeMenuIndex,
    setMenuIndex: setModeMenuIndex,
    selectPreset,
    openConfigEditor,
    moveEditField,
    adjustEditValue,
    savePreset,
    savePresetAsNew,
    deletePreset,
    stopSession,
    transitionToMode,
    moveDateRangeIndex,
    selectDateRange,
    openDateRangePicker,
    updateEditingField,
    editingConfig,
    backtestProgress,
    dismissBacktestResult,
    getDateRangePresetCount,
    stopBacktest,
    cyclePreset,
    adjustBacktestSpeed,
  } = useModeStore();

  const getDataManagerActions = () =>
    (globalThis as Record<string, unknown>).__dataManagerActions as
      | {
          startFetch?: () => void;
          cancelFetch?: () => void;
          toggleTradesPresetMode?: () => void;
          exitTradesPresetMode?: () => void;
          moveTradesPresetIndex?: (delta: number) => void;
          isFetching?: () => boolean;
          isTradesPresetMode?: () => boolean;
        }
      | undefined;

  function handleModeMenuKey(keyName: string | undefined) {
    if (keyName === "escape" || keyName === "h") {
      if (menuScreen === "mode_select") {
        closeModeMenu();
      } else if (menuScreen === "config_edit") {
        setMenuScreen("mode_select");
      } else if (menuScreen === "preset_select") {
        setMenuScreen("mode_select");
      } else if (menuScreen === "date_range") {
        setMenuScreen("config_edit");
      } else if (menuScreen === "data_manager") {
        const actions = getDataManagerActions();
        if (actions?.isFetching?.()) {
          actions.cancelFetch?.();
        } else if (actions?.isTradesPresetMode?.()) {
          actions.exitTradesPresetMode?.();
        } else {
          setMenuScreen("mode_select");
        }
      }
      return;
    }

    if (keyName === "j" || keyName === "down") {
      if (menuScreen === "mode_select") {
        moveModeMenuIndex(1);
      } else if (menuScreen === "preset_select") {
        moveModeMenuIndex(1);
      } else if (menuScreen === "config_edit") {
        moveEditField(1);
      } else if (menuScreen === "date_range") {
        moveDateRangeIndex(1);
      } else if (menuScreen === "data_manager") {
        const actions = getDataManagerActions();
        if (actions?.isTradesPresetMode?.()) {
          actions.moveTradesPresetIndex?.(1);
        } else {
          moveModeMenuIndex(1);
        }
      }
      return;
    }

    if (keyName === "k" || keyName === "up") {
      if (menuScreen === "mode_select") {
        moveModeMenuIndex(-1);
      } else if (menuScreen === "preset_select") {
        moveModeMenuIndex(-1);
      } else if (menuScreen === "config_edit") {
        moveEditField(-1);
      } else if (menuScreen === "date_range") {
        moveDateRangeIndex(-1);
      } else if (menuScreen === "data_manager") {
        const actions = getDataManagerActions();
        if (actions?.isTradesPresetMode?.()) {
          actions.moveTradesPresetIndex?.(-1);
        } else {
          moveModeMenuIndex(-1);
        }
      }
      return;
    }

    if (keyName === "+" || keyName === "=" || keyName === "right" || keyName === "l") {
      if (menuScreen === "config_edit") {
        if (editFieldIndex === EDIT_FIELDS.length + 1 && viewMode === "backtest") {
          const current = editingConfig?.backtestIntervalHours || 1;
          updateEditingField("backtestIntervalHours", Math.min(24, current + 1));
        } else {
          adjustEditValue(1);
        }
      }
      return;
    }

    if (keyName === "-" || keyName === "_" || keyName === "left" || keyName === "l") {
      if (menuScreen === "config_edit") {
        if (editFieldIndex === EDIT_FIELDS.length + 1 && viewMode === "backtest") {
          const current = editingConfig?.backtestIntervalHours || 1;
          updateEditingField("backtestIntervalHours", Math.max(1, current - 1));
        } else {
          adjustEditValue(-1);
        }
      }
      return;
    }

    if (keyName === "c") {
      if (menuScreen === "mode_select") {
        openConfigEditor();
      }
      return;
    }

    if (keyName === "p") {
      if (menuScreen === "mode_select") {
        setMenuScreen("preset_select");
        setModeMenuIndex(selectedPresetIndex);
      }
      return;
    }

    if (keyName === "d") {
      if (menuScreen === "mode_select") {
        setMenuScreen("data_manager");
      } else if (menuScreen === "preset_select" && menuIndex < configPresets.length) {
        deletePreset(menuIndex);
      } else if (menuScreen === "config_edit" && editFieldIndex === EDIT_FIELDS.length) {
        openDateRangePicker();
      }
      return;
    }

    if (keyName === "t" && menuScreen === "data_manager") {
      if (!getDataManagerActions()?.isFetching?.()) {
        getDataManagerActions()?.toggleTradesPresetMode?.();
      }
      return;
    }

    if (keyName === "e") {
      if (menuScreen === "preset_select" && menuIndex < configPresets.length) {
        openConfigEditor(configPresets[menuIndex]);
      }
      return;
    }

    if (keyName === "n") {
      if (menuScreen === "config_edit") {
        savePresetAsNew();
      }
      return;
    }

    if (keyName === "s") {
      if (menuScreen === "config_edit") {
        const currentState = useModeStore.getState();
        const isBacktest = currentState.viewMode === "backtest";
        const hasDates = currentState.editingConfig?.backtestStart && currentState.editingConfig?.backtestEnd;
        const isRunning = currentState.backtestProgress.status === "running";

        if (isBacktest) {
          if (isRunning) {
            (async () => {
              await stopBacktest();
            })();
          } else if (!hasDates) {
            openDateRangePicker();
          } else {
            (async () => {
              await stopBacktest();
              await savePreset();
              await useModeStore.getState().startBacktest();
            })();
          }
        }
      }
      return;
    }

    if (keyName && matchesKey(keyName, keybindings.enter)) {
      if (menuScreen === "mode_select") {
        const modes = ["paper", "backtest", "live"] as const;
        const selectedMode = modes[menuIndex];
        if (!selectedMode || selectedMode === "live") return;

        if (sessionStatus === "running" && viewMode === selectedMode) {
          stopSession();
        } else {
          transitionToMode(selectedMode);
        }
      } else if (menuScreen === "preset_select") {
        if (menuIndex < configPresets.length) {
          selectPreset(menuIndex);
          setMenuScreen("mode_select");
        } else {
          const basePreset = configPresets[0];
          if (basePreset) {
            openConfigEditor({
              ...basePreset,
              name: "New Preset",
            });
          }
        }
      } else if (menuScreen === "config_edit") {
        (async () => {
          await savePreset();
          closeModeMenu();
        })();
      } else if (menuScreen === "date_range") {
        if (!dataAvailability?.has_data || !dataAvailability.start_date || !dataAvailability.end_date) {
          return;
        }

        const presetCount = getDateRangePresetCount();
        const isCustom = dateRangeIndex >= presetCount;

        if (isCustom) {
          const start = customStartDate || dataAvailability.start_date;
          const end = customEndDate || dataAvailability.end_date;
          selectDateRange(start, end);
        } else {
          const availStart = dataAvailability.start_date;
          const availEnd = dataAvailability.end_date;

          if (dateRangeIndex === 0) {
            selectDateRange(availStart, availEnd);
          } else if (dataAvailability.days_count > 1) {
            const startDate = new Date(availStart + "T00:00:00");
            const endDate = new Date(availEnd + "T00:00:00");
            const daysDiff = Math.ceil((endDate.getTime() - startDate.getTime()) / (1000 * 60 * 60 * 24));
            const midDate = new Date(startDate);
            midDate.setDate(midDate.getDate() + Math.floor(daysDiff / 2));
            const midStr = midDate.toISOString().split("T")[0] || availEnd;

            if (dateRangeIndex === 1) {
              selectDateRange(availStart, midStr);
            } else if (dateRangeIndex === 2) {
              selectDateRange(midStr, availEnd);
            }
          }
        }
      } else if (menuScreen === "data_manager") {
        const actions = getDataManagerActions();
        if (!actions?.isFetching?.()) {
          actions?.startFetch?.();
        }
      }
      return;
    }
  }

  useKeyboard((key) => {
    const keyName = key.name || key.sequence;

    // dismiss backtest results/errors or cancel running backtest on escape
    if (keyName === "escape" && (backtestProgress.status === "complete" || backtestProgress.status === "failed")) {
      dismissBacktestResult();
      return;
    }
    if (keyName === "escape" && backtestProgress.status === "running") {
      stopBacktest().catch(() => {
        dismissBacktestResult();
      });
      return;
    }

    // cancel download on escape when on data tab
    if (keyName === "escape" && activeTab === "data") {
      const actions = (globalThis as Record<string, unknown>).__dataCollectorActions as
        { cancelFetch?: () => void; isFetching?: () => boolean } | undefined;
      if (actions?.isFetching?.()) {
        actions.cancelFetch?.();
        return;
      }
    }

    if (key.ctrl && key.name === "p") {
      if (menuScreen !== "closed") {
        closeModeMenu();
      } else {
        toggleMenu();
      }
      return;
    }

    if (menuScreen !== "closed") {
      handleModeMenuKey(keyName);
      return;
    }

    if (menuOpen) {
      const result = handleMenuKey(key);
      if (result.action === "reconnect") {
        options.onReconnect?.();
      } else if (result.action === "help") {
        toggleHelp();
      } else if (result.action === "modes") {
        openModeMenu();
      }
      return;
    }

    // tab switching
    if (keyName === keybindings.tabs.overview) {
      setActiveTab("overview");
      return;
    }
    if (keyName === keybindings.tabs.positions) {
      setActiveTab("positions");
      return;
    }
    if (keyName === keybindings.tabs.trades) {
      setActiveTab("trades");
      return;
    }
    if (keyName === keybindings.tabs.engine) {
      setActiveTab("engine");
      return;
    }
    if (keyName === keybindings.tabs.decisions) {
      setActiveTab("decisions");
      return;
    }
    if (keyName === keybindings.tabs.timeline) {
      setActiveTab("timeline");
      return;
    }
    if (keyName === keybindings.tabs.data) {
      setActiveTab("data");
      return;
    }

    // overview tab session controls
    if (activeTab === "overview" && menuScreen === "closed") {
      const isIdle = sessionStatus === "idle" && viewMode === "idle";
      const isRunning = sessionStatus === "running" || sessionStatus === "paused";
      const isBacktestActive = backtestProgress.status === "running";

      if (keyName === "b" && isIdle) {
        (async () => {
          await transitionToMode("backtest" as TradingMode);
        })();
        return;
      }

      if (keyName === "p" && isIdle) {
        (async () => {
          await transitionToMode("paper" as TradingMode);
        })();
        return;
      }

      if (keyName === "x" && isRunning) {
        if (isBacktestActive) {
          stopBacktest().catch(() => {});
        } else {
          stopSession();
        }
        return;
      }

      if (keyName === "space" && (sessionStatus === "running" || sessionStatus === "paused")) {
        if (engineStatus?.state === "Running") {
          pauseEngine();
        } else {
          resumeEngine();
        }
        return;
      }

      if (keyName === "," && isIdle) {
        cyclePreset(-1);
        return;
      }

      if (keyName === "." && isIdle) {
        cyclePreset(1);
        return;
      }

      if (keyName === "c") {
        openModeMenu();
        openConfigEditor();
        return;
      }

      if ((keyName === "+" || keyName === "=") && isBacktestActive) {
        adjustBacktestSpeed(1);
        return;
      }

      if ((keyName === "-" || keyName === "_") && isBacktestActive) {
        adjustBacktestSpeed(-1);
        return;
      }
    }

    // quit
    if (matchesKey(keyName, keybindings.quit)) {
      options.onQuit?.() ?? process.exit(0);
      return;
    }

    // reconnect
    if (keyName === keybindings.reconnect) {
      options.onReconnect?.();
      return;
    }

    // help
    if (keyName === keybindings.help) {
      toggleHelp();
      return;
    }

    // acknowledge alerts
    if (keyName === keybindings.acknowledgeAlerts) {
      acknowledgeAllAlerts();
      return;
    }

    // navigation
    if (matchesKey(keyName, keybindings.down)) {
      moveSelection(1);
      return;
    }
    if (matchesKey(keyName, keybindings.up)) {
      moveSelection(-1);
      return;
    }
    if (keyName === keybindings.first) {
      moveSelection(-Infinity);
      return;
    }
    if (keyName === keybindings.last) {
      moveSelection(Infinity);
      return;
    }

    // start download (data tab)
    if (matchesKey(keyName, keybindings.enter) && activeTab === "data") {
      const actions = (globalThis as Record<string, unknown>).__dataCollectorActions as
        { startFetch?: () => void; isFetching?: () => boolean } | undefined;
      if (actions && !actions.isFetching?.()) {
        actions.startFetch?.();
      }
      return;
    }

    // enter/drill-down
    if (matchesKey(keyName, keybindings.enter)) {
      if (activeTab === "positions" && positions[selectedIndex]) {
        setSelectedMarket(positions[selectedIndex].ticker);
        setActiveTab("drilldown");
      } else if (activeTab === "engine") {
        const allSpecimens = beds.flatMap((b) => b.specimens);
        if (allSpecimens[selectedIndex]) {
          setSelectedSpecimen(allSpecimens[selectedIndex].name);
          setActiveTab("drilldown");
        }
      }
      return;
    }

    // back
    if (keyName === keybindings.back) {
      if (activeTab === "drilldown") {
        setActiveTab("overview");
      }
      return;
    }

    // toggle view mode (positions tab)
    if (keyName === "v" && activeTab === "positions") {
      togglePositionsViewMode();
      return;
    }

    // pause/resume (engine tab)
    if (keyName === keybindings.pause && activeTab === "engine") {
      if (engineStatus?.state === "Running") {
        pauseEngine();
      } else {
        resumeEngine();
      }
      return;
    }

    // cycle trades/day preset (data tab)
    if (keyName === keybindings.toggle && activeTab === "data") {
      const actions = (globalThis as Record<string, unknown>).__dataCollectorActions as
        { cycleTradesPreset?: () => void } | undefined;
      actions?.cycleTradesPreset?.();
      return;
    }

    // toggle scorer (engine tab)
    if (keyName === keybindings.toggle && activeTab === "engine") {
      const allSpecimens = beds.flatMap((b) => b.specimens);
      if (allSpecimens[selectedIndex]) {
        toggleSpecimen(allSpecimens[selectedIndex].name);
      }
      return;
    }

    // weight adjustment (engine tab)
    if (keyName === keybindings.weightUp && activeTab === "engine") {
      const allSpecimens = beds.flatMap((b) => b.specimens);
      if (allSpecimens[selectedIndex]) {
        adjustSpecimenWeight(allSpecimens[selectedIndex].name, 0.05);
      }
      return;
    }
    if (keyName === keybindings.weightDown && activeTab === "engine") {
      const allSpecimens = beds.flatMap((b) => b.specimens);
      if (allSpecimens[selectedIndex]) {
        adjustSpecimenWeight(allSpecimens[selectedIndex].name, -0.05);
      }
      return;
    }
  });
}
