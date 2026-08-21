import os
import time
from getpass import getpass
from urllib.parse import quote

import cv2
from onvif import ONVIFCamera

BACKGROUND_LEARNING_FRAMES = 90
MIN_MOTION_AREA = 1_000
RTSP_HOST = "192.168.1.65"
RTSP_PORT = 554
ONVIF_PORT = 2020
RTSP_USERNAME = "michalmazak"
PTZ_SPEED = 0.5
PTZ_MOVE_DURATION_SECONDS = 0.2

LEFT_KEYS = {81, 65361, 63234, 2424832}
UP_KEYS = {82, 65362, 63232, 2490368}
RIGHT_KEYS = {83, 65363, 63235, 2555904}
DOWN_KEYS = {84, 65364, 63233, 2621440}


class PtzController:
    def __init__(self, username: str, password: str) -> None:
        camera = ONVIFCamera(RTSP_HOST, ONVIF_PORT, username, password)
        media_service = camera.create_media_service()
        profiles = media_service.GetProfiles()
        if not profiles:
            raise RuntimeError("The camera did not provide an ONVIF media profile.")

        self.ptz_service = camera.create_ptz_service()
        self.profile_token = profiles[0].token

    def move(self, pan: float, tilt: float) -> None:
        request = self.ptz_service.create_type("ContinuousMove")
        request.ProfileToken = self.profile_token
        request.Velocity = {"PanTilt": {"x": pan, "y": tilt}}
        self.ptz_service.ContinuousMove(request)

    def stop(self) -> None:
        request = self.ptz_service.create_type("Stop")
        request.ProfileToken = self.profile_token
        request.PanTilt = True
        request.Zoom = True
        self.ptz_service.Stop(request)


def get_camera_credentials() -> tuple[str, str]:
    password = os.environ.get("RTSP_PASSWORD") or getpass("Camera password: ")
    return RTSP_USERNAME, password


def get_rtsp_url(username: str, password: str) -> str:
    rtsp_url = os.environ.get("RTSP_URL")
    if rtsp_url:
        return rtsp_url

    path = os.environ.get("RTSP_PATH")
    if not path:
        raise RuntimeError("Set RTSP_URL or RTSP_PATH before running.")

    return (
        f"rtsp://{quote(username, safe='')}:{quote(password, safe='')}"
        f"@{RTSP_HOST}:{RTSP_PORT}/{path.lstrip('/')}"
    )


def open_stream(rtsp_url: str) -> cv2.VideoCapture:
    stream = cv2.VideoCapture(rtsp_url, cv2.CAP_FFMPEG)
    if stream.isOpened():
        return stream

    stream.release()
    return cv2.VideoCapture(rtsp_url)


def create_background_subtractor() -> cv2.BackgroundSubtractor:
    return cv2.createBackgroundSubtractorMOG2(
        history=500,
        varThreshold=32,
        detectShadows=True,
    )


def get_ptz_velocity(key: int) -> tuple[float, float] | None:
    if key in LEFT_KEYS:
        return -PTZ_SPEED, 0.0
    if key in RIGHT_KEYS:
        return PTZ_SPEED, 0.0
    if key in UP_KEYS:
        return 0.0, PTZ_SPEED
    if key in DOWN_KEYS:
        return 0.0, -PTZ_SPEED
    return None


def main() -> None:
    username, password = get_camera_credentials()
    stream = open_stream(get_rtsp_url(username, password))
    if not stream.isOpened():
        raise RuntimeError("Could not open the RTSP stream.")

    ptz_controller = PtzController(username, password)
    window_name = "RTSP motion detection — arrows: PTZ, S: stop, Q/Esc: close, R: reset"
    background_subtractor = create_background_subtractor()
    learning_frames_remaining = BACKGROUND_LEARNING_FRAMES
    is_moving = False
    stop_moving_at = 0.0

    try:
        while True:
            success, frame = stream.read()
            if not success:
                raise RuntimeError("Could not read a frame from the RTSP stream.")

            foreground_mask = background_subtractor.apply(frame)

            if learning_frames_remaining > 0:
                learning_frames_remaining -= 1
                cv2.putText(
                    frame,
                    "Learning background...",
                    (20, 40),
                    cv2.FONT_HERSHEY_SIMPLEX,
                    0.8,
                    (0, 255, 255),
                    2,
                )
            else:
                _, foreground_mask = cv2.threshold(
                    foreground_mask, 200, 255, cv2.THRESH_BINARY
                )
                kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
                foreground_mask = cv2.morphologyEx(
                    foreground_mask, cv2.MORPH_OPEN, kernel
                )
                foreground_mask = cv2.dilate(foreground_mask, None, iterations=2)

                contours, _ = cv2.findContours(
                    foreground_mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE
                )
                motion_detected = False
                for contour in contours:
                    if cv2.contourArea(contour) < MIN_MOTION_AREA:
                        continue

                    motion_detected = True
                    x, y, width, height = cv2.boundingRect(contour)
                    cv2.rectangle(
                        frame,
                        (x, y),
                        (x + width, y + height),
                        (0, 255, 0),
                        2,
                    )

                if motion_detected:
                    cv2.putText(
                        frame,
                        "Motion detected",
                        (20, 40),
                        cv2.FONT_HERSHEY_SIMPLEX,
                        0.8,
                        (0, 0, 255),
                        2,
                    )

            mask_preview = cv2.cvtColor(foreground_mask, cv2.COLOR_GRAY2BGR)
            cv2.putText(
                frame,
                "RTSP stream",
                (20, frame.shape[0] - 20),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.7,
                (255, 255, 255),
                2,
            )
            cv2.putText(
                mask_preview,
                "MOG2 mask",
                (20, mask_preview.shape[0] - 20),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.7,
                (255, 255, 255),
                2,
            )
            cv2.imshow(window_name, cv2.hconcat([frame, mask_preview]))

            if is_moving and time.monotonic() >= stop_moving_at:
                ptz_controller.stop()
                is_moving = False

            key = cv2.waitKeyEx(1)
            character = key & 0xFF
            velocity = get_ptz_velocity(key)
            if velocity is not None:
                ptz_controller.move(*velocity)
                is_moving = True
                stop_moving_at = time.monotonic() + PTZ_MOVE_DURATION_SECONDS
                background_subtractor = create_background_subtractor()
                learning_frames_remaining = BACKGROUND_LEARNING_FRAMES
            elif character in (ord("q"), 27):
                break
            elif character == ord("s"):
                ptz_controller.stop()
                is_moving = False
            elif character == ord("r"):
                background_subtractor = create_background_subtractor()
                learning_frames_remaining = BACKGROUND_LEARNING_FRAMES
    finally:
        if is_moving:
            ptz_controller.stop()
        stream.release()
        cv2.destroyAllWindows()


if __name__ == "__main__":
    main()
