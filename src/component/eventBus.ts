import mitt, {Emitter} from 'mitt';

// types/eventTypes.ts


type BusEvents = {
    "state-message": any;
}

export const eventBus: Emitter<BusEvents> = mitt<BusEvents>();
