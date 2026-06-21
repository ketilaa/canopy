import { Logger } from './logger';

export interface NotificationService {
    send(message: string): void;
}

export class EmailNotifier implements NotificationService {
    send(message: string): void {
        Logger.log(message);
    }
}

export enum NotificationChannel {
    Email,
    Sms,
    Push,
}

export function createNotifier(channel: NotificationChannel): NotificationService {
    return new EmailNotifier();
}
