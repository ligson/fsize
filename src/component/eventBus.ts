import mitt, {Emitter} from 'mitt';

// types/eventTypes.ts


type BusEvents = {
    "state-message": {text: string};
    "scan-progress": {path: string; entriesScanned: number; elapsedMs: number};
}

export const eventBus: Emitter<BusEvents> = mitt<BusEvents>();
