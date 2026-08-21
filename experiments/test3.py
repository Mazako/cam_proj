import os
import time
from getpass import getpass
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlretrieve

import cv2
import numpy as np
import onnxruntime as ort
from onvif import ONVIFCamera


RTSP_HOST = "192.168.1.65"
RTSP_PORT = 554
ONVIF_PORT = 2020
RTSP_USERNAME = "michalmazak"
MODEL_PATH = Path(__file__).parent.parent / "yolo26n.onnx"
MODEL_URL = "https://github.com/ultralytics/assets/releases/download/v8.4.0/yolo26n.onnx"
PERSON_CLASS_ID = 0
CONFIDENCE_THRESHOLD = 0.5
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


class PersonDetector:
    def __init__(self, model_path: Path) -> None:
        self.session = ort.InferenceSession(
            str(model_path), providers=ort.get_available_providers()
        )
        model_input = self.session.get_inputs()[0]
        self.input_name = model_input.name
        self.input_height = self.get_dimension(model_input.shape[2])
        self.input_width = self.get_dimension(model_input.shape[3])

    @staticmethod
    def get_dimension(value: int | str | None) -> int:
        return value if isinstance(value, int) else 640

    def detect(self, frame: np.ndarray) -> list[tuple[int, int, int, int, float]]:
        tensor, scale, pad_x, pad_y = self.prepare_input(frame)
        output = self.session.run(None, {self.input_name: tensor})[0]
        detections = np.squeeze(output, axis=0)
        if detections.ndim != 2 or detections.shape[1] != 6:
            raise RuntimeError("The ONNX model must use end-to-end detection with NMS.")

        height, width = frame.shape[:2]
        people = []
        for x1, y1, x2, y2, confidence, class_id in detections:
            if int(class_id) != PERSON_CLASS_ID or confidence < CONFIDENCE_THRESHOLD:
                continue

            left = int(np.clip((x1 - pad_x) / scale, 0, width - 1))
            top = int(np.clip((y1 - pad_y) / scale, 0, height - 1))
            right = int(np.clip((x2 - pad_x) / scale, 0, width - 1))
            bottom = int(np.clip((y2 - pad_y) / scale, 0, height - 1))
            people.append((left, top, right, bottom, float(confidence)))

        return people

    def prepare_input(
        self, frame: np.ndarray
    ) -> tuple[np.ndarray, float, int, int]:
        frame_height, frame_width = frame.shape[:2]
        scale = min(self.input_width / frame_width, self.input_height / frame_height)
        resized_width = round(frame_width * scale)
        resized_height = round(frame_height * scale)
        resized = cv2.resize(frame, (resized_width, resized_height))
        canvas = np.full(
            (self.input_height, self.input_width, 3), 114, dtype=np.uint8
        )
        pad_x = (self.input_width - resized_width) // 2
        pad_y = (self.input_height - resized_height) // 2
        canvas[
            pad_y : pad_y + resized_height,
            pad_x : pad_x + resized_width,
        ] = resized
        rgb_image = cv2.cvtColor(canvas, cv2.COLOR_BGR2RGB)
        tensor = np.ascontiguousarray(
            rgb_image.transpose(2, 0, 1)[np.newaxis], dtype=np.float32
        )
        return tensor / 255.0, scale, pad_x, pad_y


def get_camera_credentials() -> tuple[str, str]:
    password = os.environ.get("RTSP_PASSWORD") or getpass("Camera password: ")
    return RTSP_USERNAME, password


def get_rtsp_url(username: str, password: str) -> str:
    rtsp_url = os.environ.get("RTSP_URL")
    if rtsp_url:
        return rtsp_url

    path = os.environ.get("RTSP_PATH", "stream1")
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


def ensure_model() -> Path:
    if MODEL_PATH.exists():
        return MODEL_PATH

    try:
        urlretrieve(MODEL_URL, MODEL_PATH)
    except OSError as error:
        raise RuntimeError(f"Could not download the YOLO model: {error}") from error
    return MODEL_PATH


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
    detector = PersonDetector(ensure_model())
    stream = open_stream(get_rtsp_url(username, password))
    if not stream.isOpened():
        raise RuntimeError("Could not open the RTSP stream.")

    ptz_controller = PtzController(username, password)
    window_name = "YOLO people detection — arrows: PTZ, S: stop, Q/Esc: close"
    is_moving = False
    stop_moving_at = 0.0

    try:
        while True:
            success, frame = stream.read()
            if not success:
                raise RuntimeError("Could not read a frame from the RTSP stream.")

            people = detector.detect(frame)
            for left, top, right, bottom, confidence in people:
                cv2.rectangle(frame, (left, top), (right, bottom), (0, 255, 0), 2)
                cv2.putText(
                    frame,
                    f"Person {confidence:.0%}",
                    (left, max(top - 10, 25)),
                    cv2.FONT_HERSHEY_SIMPLEX,
                    0.7,
                    (0, 255, 0),
                    2,
                )

            cv2.putText(
                frame,
                f"People: {len(people)}",
                (20, 40),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.8,
                (0, 255, 255),
                2,
            )
            cv2.imshow(window_name, frame)

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
            elif character in (ord("q"), 27):
                break
            elif character == ord("s"):
                ptz_controller.stop()
                is_moving = False
    finally:
        if is_moving:
            ptz_controller.stop()
        stream.release()
        cv2.destroyAllWindows()


if __name__ == "__main__":
    main()
